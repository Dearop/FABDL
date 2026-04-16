"""Generic Ethereum JSON-RPC client.

Vendor-neutral by design: speaks only standard ``eth_*`` methods so any endpoint
(paid, free, self-hosted, local Anvil fork) works. No ``alchemy_*``, ``trace_*``,
or provider-proprietary extensions — use capability detection instead.

The response cache is keyed on ``(chain_id, method, block, target, calldata)``
and stores only immutable results (``eth_call`` at a specific block, finalized
``eth_getBlockByNumber``). It is a plain SQLite file so concurrent workers
don't corrupt it.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import sqlite3
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import requests
from eth_abi import decode as abi_decode
from eth_abi import encode as abi_encode
from tenacity import (
    retry,
    retry_if_exception_type,
    stop_after_attempt,
    wait_exponential,
)

from fabdl.core.constants import MULTICALL3_ADDRESS

log = logging.getLogger(__name__)


class RpcError(Exception):
    """Non-retryable JSON-RPC error from the endpoint."""


class RpcTransientError(Exception):
    """Retryable transport or rate-limit error."""


class TooManyResultsError(RpcError):
    """``eth_getLogs`` exceeded the provider's result cap; caller should halve the range."""


@dataclass
class Call:
    target: str  # 0x-prefixed checksum or lowercase address
    data: bytes  # abi-encoded calldata


@dataclass
class RpcConfig:
    url: str
    archive_url: str | None = None
    max_logs_per_request: int = 10_000
    initial_log_chunk_blocks: int = 2_000
    rate_limit_rps: float | None = None
    retry_attempts: int = 5
    retry_min_wait: float = 1.0
    retry_max_wait: float = 16.0
    request_headers: dict[str, str] = field(default_factory=dict)
    request_timeout_seconds: float = 30.0
    cache_path: Path | None = None  # defaults to data/checkpoints/rpc_cache.sqlite

    @classmethod
    def from_env(cls) -> RpcConfig:
        url = os.environ.get("RPC_URL")
        if not url:
            raise RuntimeError("RPC_URL is not set — see .env.example")
        return cls(
            url=url,
            archive_url=os.environ.get("ARCHIVE_RPC_URL") or None,
            max_logs_per_request=int(os.environ.get("RPC_MAX_LOGS_PER_REQUEST") or 10_000),
            initial_log_chunk_blocks=int(os.environ.get("RPC_INITIAL_LOG_CHUNK_BLOCKS") or 2_000),
            rate_limit_rps=float(os.environ["RPC_RATE_LIMIT_RPS"])
            if os.environ.get("RPC_RATE_LIMIT_RPS")
            else None,
        )


class _RpcCache:
    """SQLite-backed read-through cache for immutable RPC responses."""

    def __init__(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self._path = path
        self._lock = threading.Lock()
        with self._connect() as conn:
            conn.execute(
                "CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, value BLOB NOT NULL)"
            )

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(self._path, timeout=30.0)
        conn.execute("PRAGMA journal_mode=WAL")
        return conn

    def get(self, key: str) -> bytes | None:
        with self._lock, self._connect() as conn:
            row = conn.execute("SELECT value FROM cache WHERE key = ?", (key,)).fetchone()
        return row[0] if row else None

    def put(self, key: str, value: bytes) -> None:
        with self._lock, self._connect() as conn:
            conn.execute(
                "INSERT OR REPLACE INTO cache (key, value) VALUES (?, ?)", (key, value)
            )


class _RateLimiter:
    def __init__(self, rps: float | None):
        self._min_interval = 1.0 / rps if rps else 0.0
        self._lock = threading.Lock()
        self._next_allowed = 0.0

    def wait(self) -> None:
        if self._min_interval == 0.0:
            return
        with self._lock:
            now = time.monotonic()
            wait = self._next_allowed - now
            if wait > 0:
                time.sleep(wait)
            self._next_allowed = max(now, self._next_allowed) + self._min_interval


def _hex_to_int(h: str) -> int:
    return int(h, 16) if isinstance(h, str) else h


def _int_to_hex(i: int) -> str:
    return hex(i)


def _block_to_rpc(block: int | str) -> str:
    if isinstance(block, int):
        return _int_to_hex(block)
    if block in ("latest", "earliest", "pending", "safe", "finalized"):
        return block
    raise ValueError(f"bad block reference: {block!r}")


class RpcClient:
    """Standards-only JSON-RPC client. One instance per endpoint."""

    def __init__(self, config: RpcConfig, *, data_dir: Path | None = None):
        self._cfg = config
        self._session = requests.Session()
        if config.request_headers:
            self._session.headers.update(config.request_headers)
        self._session.headers["content-type"] = "application/json"
        self._archive_session = (
            self._session  # same underlying session; URL chooses routing
            if not config.archive_url
            else requests.Session()
        )
        if config.archive_url and config.request_headers:
            self._archive_session.headers.update(config.request_headers)

        cache_path = config.cache_path or (
            (data_dir or Path("data")) / "checkpoints" / "rpc_cache.sqlite"
        )
        self._cache = _RpcCache(cache_path)
        self._limiter = _RateLimiter(config.rate_limit_rps)

        self._chain_id: int | None = None
        self._supports_archive: bool | None = None

    # ------------------------------------------------------------------ transport

    def _post(self, url: str, payload: dict[str, Any] | list[dict[str, Any]]) -> Any:
        self._limiter.wait()

        @retry(
            retry=retry_if_exception_type(RpcTransientError),
            wait=wait_exponential(min=self._cfg.retry_min_wait, max=self._cfg.retry_max_wait),
            stop=stop_after_attempt(self._cfg.retry_attempts),
            reraise=True,
        )
        def _do() -> Any:
            try:
                resp = self._session.post(
                    url, json=payload, timeout=self._cfg.request_timeout_seconds
                )
            except requests.RequestException as exc:
                raise RpcTransientError(f"transport error: {exc}") from exc
            if resp.status_code == 429 or 500 <= resp.status_code < 600:
                raise RpcTransientError(f"HTTP {resp.status_code}: {resp.text[:200]}")
            if resp.status_code >= 400:
                raise RpcError(f"HTTP {resp.status_code}: {resp.text[:200]}")
            return resp.json()

        return _do()

    def _rpc(self, method: str, params: list[Any], *, archive: bool = False) -> Any:
        url = self._cfg.archive_url if archive and self._cfg.archive_url else self._cfg.url
        payload = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
        data = self._post(url, payload)
        if "error" in data:
            err = data["error"]
            msg = err.get("message", str(err))
            low = msg.lower()
            if "too many" in low or "query returned more than" in low or "response size" in low:
                raise TooManyResultsError(msg)
            if any(h in low for h in ("rate limit", "timeout", "temporarily", "overloaded")):
                raise RpcTransientError(msg)
            raise RpcError(msg)
        return data["result"]

    # ------------------------------------------------------------------ methods

    def chain_id(self) -> int:
        if self._chain_id is None:
            self._chain_id = _hex_to_int(self._rpc("eth_chainId", []))
        return self._chain_id

    def get_block_number(self) -> int:
        return _hex_to_int(self._rpc("eth_blockNumber", []))

    def get_block(self, number: int | str, *, full_transactions: bool = False) -> dict:
        block_ref = _block_to_rpc(number)
        cacheable = isinstance(number, int)
        key = (
            self._cache_key("eth_getBlockByNumber", block_ref, "", full_transactions)
            if cacheable
            else None
        )
        if key is not None:
            hit = self._cache.get(key)
            if hit is not None:
                return json.loads(hit)
        result = self._rpc("eth_getBlockByNumber", [block_ref, full_transactions])
        if key is not None and result is not None:
            self._cache.put(key, json.dumps(result).encode())
        return result

    def call(self, to: str, data: bytes, block: int | str = "latest") -> bytes:
        block_ref = _block_to_rpc(block)
        is_historical = isinstance(block, int)
        key = (
            self._cache_key("eth_call", block_ref, to.lower(), data.hex())
            if is_historical
            else None
        )
        if key is not None:
            hit = self._cache.get(key)
            if hit is not None:
                return hit
        params = [{"to": to, "data": "0x" + data.hex()}, block_ref]
        result_hex: str = self._rpc("eth_call", params, archive=is_historical)
        result = bytes.fromhex(result_hex[2:])
        if key is not None:
            self._cache.put(key, result)
        return result

    def get_logs(
        self,
        address: str,
        topics: list[str | None | list[str]],
        from_block: int,
        to_block: int,
    ) -> list[dict]:
        params = [
            {
                "address": address,
                "topics": topics,
                "fromBlock": _int_to_hex(from_block),
                "toBlock": _int_to_hex(to_block),
            }
        ]
        return self._rpc("eth_getLogs", params, archive=True)

    def multicall(
        self,
        calls: list[Call],
        *,
        block: int | str = "latest",
        allow_failure: bool = False,
    ) -> list[bytes]:
        """Aggregate3 via Multicall3. Standard `eth_call`, works on any endpoint."""
        selector = bytes.fromhex("82ad56cb")  # aggregate3((address,bool,bytes)[])
        tuples = [(c.target, allow_failure, c.data) for c in calls]
        encoded_args = abi_encode(["(address,bool,bytes)[]"], [tuples])
        data = selector + encoded_args
        result = self.call(MULTICALL3_ADDRESS, data, block=block)
        (decoded,) = abi_decode(["(bool,bytes)[]"], result)
        out: list[bytes] = []
        for success, ret in decoded:
            if not success and not allow_failure:
                raise RpcError("multicall call failed")
            out.append(ret)
        return out

    # ------------------------------------------------------------------ introspection

    @property
    def supports_archive(self) -> bool:
        """True if the endpoint can serve ``eth_call`` at a block more than 128 behind head.

        Probed lazily on first access — costs one ``eth_call``.
        """
        if self._supports_archive is None:
            self._supports_archive = self._probe_archive()
        return self._supports_archive

    def _probe_archive(self) -> bool:
        try:
            head = self.get_block_number()
        except RpcError:
            return False
        probe_block = max(1, head - 1_000)
        try:
            # Call chainId() on multicall3 at a historical block — cheapest possible probe.
            self.call(
                MULTICALL3_ADDRESS,
                bytes.fromhex("3408e470"),  # getChainId()
                block=probe_block,
            )
            return True
        except (RpcError, RpcTransientError) as exc:
            log.info("archive probe failed (%s) — Path B event-replay will be used", exc)
            return False

    # ------------------------------------------------------------------ helpers

    def _cache_key(self, method: str, block: str, target: str, payload: Any) -> str:
        chain_id = self.chain_id()
        raw = f"{chain_id}|{method}|{block}|{target}|{payload}".encode()
        return hashlib.sha256(raw).hexdigest()
