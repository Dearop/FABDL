"""Offline tests for the RPC client.

We fake HTTP with monkeypatch — this verifies the provider-agnostic contract
(only ``eth_*`` methods, standard payload shape, caching, error routing) without
hitting a real endpoint.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from fabdl.io.rpc import (
    Call,
    RpcClient,
    RpcConfig,
    RpcError,
    RpcTransientError,
    TooManyResultsError,
)


class FakeResponse:
    def __init__(self, payload: Any, status: int = 200):
        self._payload = payload
        self.status_code = status
        self.text = json.dumps(payload) if isinstance(payload, (dict, list)) else str(payload)

    def json(self) -> Any:
        return self._payload


class FakeSession:
    """Records requests and replays canned responses."""

    def __init__(self, responses: list[Any]):
        self._queue = list(responses)
        self.calls: list[dict] = []
        self.headers: dict[str, str] = {}

    def post(self, url: str, *, json: dict[str, Any], timeout: float) -> FakeResponse:
        self.calls.append({"url": url, "json": json, "timeout": timeout})
        if not self._queue:
            raise AssertionError("no canned response for call")
        item = self._queue.pop(0)
        if isinstance(item, Exception):
            raise item
        return item


@pytest.fixture
def client(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[RpcClient, FakeSession]:
    sess = FakeSession([])
    monkeypatch.setattr("fabdl.io.rpc.requests.Session", lambda: sess)
    cfg = RpcConfig(
        url="http://fake",
        cache_path=tmp_path / "cache.sqlite",
        retry_attempts=1,
    )
    c = RpcClient(cfg, data_dir=tmp_path)
    return c, sess


def test_chain_id_uses_standard_method(client):
    c, sess = client
    sess._queue.append(FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x1"}))
    assert c.chain_id() == 1
    assert sess.calls[0]["json"]["method"] == "eth_chainId"


def test_eth_call_cached_at_historical_block(client):
    c, sess = client
    sess._queue.append(FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x1"}))  # chain_id
    sess._queue.append(FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0xcafe"}))
    out = c.call("0x" + "11" * 20, b"\x12\x34", block=12_000_000)
    assert out == bytes.fromhex("cafe")
    # Second call at the same block must hit cache — no new HTTP request.
    n_calls_before = len(sess.calls)
    out2 = c.call("0x" + "11" * 20, b"\x12\x34", block=12_000_000)
    assert out2 == out
    assert len(sess.calls) == n_calls_before


def test_eth_call_at_latest_is_not_cached(client):
    c, sess = client
    sess._queue.extend(
        [
            FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x1"}),      # chain_id
            FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x0a"}),
            FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x0b"}),
        ]
    )
    c.chain_id()
    r1 = c.call("0x" + "11" * 20, b"", block="latest")
    r2 = c.call("0x" + "11" * 20, b"", block="latest")
    assert r1 != r2  # not cached — both hit the endpoint


def test_too_many_results_raises_dedicated_error(client):
    c, sess = client
    sess._queue.append(
        FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "error": {"code": -32005, "message": "query returned more than 10000 results"},
            }
        )
    )
    with pytest.raises(TooManyResultsError):
        c.get_logs("0x" + "aa" * 20, [None], 0, 100_000)


def test_non_standard_error_raises_rpc_error(client):
    c, sess = client
    sess._queue.append(
        FakeResponse({"jsonrpc": "2.0", "id": 1, "error": {"code": -32000, "message": "bad"}})
    )
    with pytest.raises(RpcError):
        c.get_block_number()


def test_rate_limit_classified_as_transient(client):
    c, sess = client
    sess._queue.append(
        FakeResponse(
            {"jsonrpc": "2.0", "id": 1, "error": {"code": -32005, "message": "rate limit exceeded"}}
        )
    )
    with pytest.raises(RpcTransientError):
        c.get_block_number()


def test_multicall_uses_standard_eth_call(client):
    """Multicall3 is encoded as a plain eth_call — no provider-specific batch API."""
    c, sess = client
    from eth_abi import encode

    sess._queue.append(FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x1"}))  # chain_id
    fake_result = encode(["(bool,bytes)[]"], [[(True, b"\xde\xad")]])
    sess._queue.append(
        FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x" + fake_result.hex()})
    )
    results = c.multicall([Call(target="0x" + "22" * 20, data=b"\x00")], block=12_000_000)
    assert results == [b"\xde\xad"]
    # Verify it was a plain eth_call against Multicall3.
    last = sess.calls[-1]["json"]
    assert last["method"] == "eth_call"
    assert last["params"][0]["to"].lower() == "0xca11bde05977b3631167028862bE2a173976CA11".lower()
