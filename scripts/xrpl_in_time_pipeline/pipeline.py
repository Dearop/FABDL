#!/usr/bin/env python3
import argparse
import json
import math
import time
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


AMM_TX_TYPES = {
    "AMMCreate",
    "AMMDeposit",
    "AMMWithdraw",
    "AMMVote",
    "AMMBid",
    "AMMDelete",
    "AMMClawback",
}

DEX_TX_TYPES = {"OfferCreate", "OfferCancel"}
XRPL_EPOCH_OFFSET = 946684800


@dataclass(frozen=True)
class Asset:
    kind: str
    currency: str = "XRP"
    issuer: str = ""

    def key(self) -> str:
        if self.kind == "xrp":
            return "XRP"
        return f"{self.currency}:{self.issuer}"


class XRPLRPC:
    def __init__(self, url: str, timeout: int = 30) -> None:
        self.url = url
        self.timeout = timeout

    def call(self, method: str, params: Dict[str, Any]) -> Dict[str, Any]:
        payload = {"method": method, "params": [params]}
        body = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            self.url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            parsed = json.loads(resp.read().decode("utf-8"))
        result = parsed.get("result", {})
        status = result.get("status")
        if status and status != "success":
            raise RuntimeError(f"{method} failed: {result}")
        return result

    def latest_validated_ledger(self) -> int:
        result = self.call("ledger", {"ledger_index": "validated"})
        ledger = result.get("ledger", {})
        idx = ledger.get("ledger_index")
        if idx is None:
            raise RuntimeError(f"Missing ledger_index in result: {result}")
        return int(idx)

    def ledger_with_transactions(self, ledger_index: int) -> Dict[str, Any]:
        return self.call(
            "ledger",
            {
                "ledger_index": ledger_index,
                "transactions": True,
                "expand": True,
                "binary": False,
            },
        )

    def amm_info(self, asset_0: Asset, asset_1: Asset, ledger_index: int) -> Dict[str, Any]:
        return self.call(
            "amm_info",
            {
                "asset": xrpl_asset_param(asset_0),
                "asset2": xrpl_asset_param(asset_1),
                "ledger_index": ledger_index,
            },
        )

    def amm_info_by_account(self, amm_account: str, ledger_index: int) -> Dict[str, Any]:
        return self.call(
            "amm_info",
            {
                "amm_account": amm_account,
                "ledger_index": ledger_index,
            },
        )

    def ledger_amm_objects(self, ledger_index: int) -> List[Dict[str, Any]]:
        amms: List[Dict[str, Any]] = []
        marker: Optional[Any] = None
        while True:
            params: Dict[str, Any] = {
                "ledger_index": ledger_index,
                "type": "amm",
                "binary": False,
                "limit": 400,
            }
            if marker is not None:
                params["marker"] = marker
            result = self.call("ledger_data", params)
            # rippled/clio can return under "state", "ledger_data", or "objects"
            entries = result.get("state") or result.get("ledger_data") or result.get("objects") or []
            for entry in entries:
                obj = entry if isinstance(entry, dict) else {}
                if obj.get("LedgerEntryType") == "AMM":
                    amms.append(obj)
            marker = result.get("marker")
            if not marker:
                break
        return amms

    def account_lines(self, account: str, ledger_index: int) -> List[Dict[str, Any]]:
        lines: List[Dict[str, Any]] = []
        marker: Optional[Any] = None
        while True:
            params: Dict[str, Any] = {"account": account, "ledger_index": ledger_index, "limit": 400}
            if marker is not None:
                params["marker"] = marker
            result = self.call("account_lines", params)
            lines.extend(result.get("lines", []))
            marker = result.get("marker")
            if not marker:
                break
        return lines


def xrpl_asset_param(asset: Asset) -> Dict[str, Any]:
    if asset.kind == "xrp":
        return {"currency": "XRP"}
    return {"currency": asset.currency, "issuer": asset.issuer}


def read_json(path: Path) -> Dict[str, Any]:
    return json.loads(path.read_text())


def write_json(path: Path, data: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=False) + "\n")


def parse_asset(data: Dict[str, Any]) -> Asset:
    kind = data.get("kind", "").lower()
    if kind == "xrp":
        return Asset(kind="xrp")
    if kind == "issued":
        return Asset(kind="issued", currency=data["currency"], issuer=data["issuer"])
    raise ValueError(f"Unsupported asset kind: {kind}")


def parse_asset_from_xrpl_obj(value: Dict[str, Any]) -> Asset:
    # XRPL object form for XRP may be {"currency":"XRP"} or fully omitted in some contexts.
    currency = value.get("currency", "XRP")
    issuer = value.get("issuer", "")
    if currency == "XRP" and not issuer:
        return Asset(kind="xrp")
    return Asset(kind="issued", currency=currency, issuer=issuer)


def _parse_floatish(value: Any) -> float:
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return 0.0
    if isinstance(value, dict):
        try:
            return float(value.get("value", "0"))
        except ValueError:
            return 0.0
    return 0.0


def discover_pools_from_ledger(rpc: XRPLRPC, snapshot_ledger: int, target_pool_count: int) -> List[Dict[str, Any]]:
    raw_amms = rpc.ledger_amm_objects(snapshot_ledger)
    discovered: List[Tuple[float, Dict[str, Any]]] = []

    for obj in raw_amms:
        asset_raw = obj.get("Asset") or obj.get("asset")
        asset2_raw = obj.get("Asset2") or obj.get("asset2")
        amm_account = obj.get("Account") or obj.get("account")
        if not isinstance(asset_raw, dict) or not isinstance(asset2_raw, dict) or not amm_account:
            continue

        asset_0 = parse_asset_from_xrpl_obj(asset_raw)
        asset_1 = parse_asset_from_xrpl_obj(asset2_raw)
        # Heuristic ranking: prefer pools with larger LP token supply.
        lp_score = _parse_floatish(obj.get("LPTokenBalance"))
        reserve_score = _parse_floatish(obj.get("Amount")) + _parse_floatish(obj.get("Amount2"))
        score = lp_score if lp_score > 0 else reserve_score

        discovered.append(
            (
                score,
                {
                    "name": f"{asset_0.key()}-{asset_1.key()}",
                    "asset_0": {"kind": asset_0.kind, "currency": asset_0.currency, "issuer": asset_0.issuer},
                    "asset_1": {"kind": asset_1.kind, "currency": asset_1.currency, "issuer": asset_1.issuer},
                    "amm_account": amm_account,
                },
            )
        )

    discovered.sort(key=lambda item: item[0], reverse=True)
    top = [item[1] for item in discovered[:target_pool_count]]
    return top


def amount_to_asset_key(value: Any) -> Optional[str]:
    if isinstance(value, str):
        return "XRP"
    if isinstance(value, dict):
        ccy = value.get("currency")
        issuer = value.get("issuer")
        if ccy and issuer:
            return f"{ccy}:{issuer}"
    return None


def tx_touches_pool(tx: Dict[str, Any], pool_assets: set) -> bool:
    tx_type = tx.get("TransactionType")
    if tx_type in AMM_TX_TYPES:
        a = tx.get("Asset")
        a2 = tx.get("Asset2")
        keys = set()
        if isinstance(a, dict):
            c, i = a.get("currency"), a.get("issuer")
            if c and i:
                keys.add(f"{c}:{i}")
        if isinstance(a2, dict):
            c, i = a2.get("currency"), a2.get("issuer")
            if c and i:
                keys.add(f"{c}:{i}")
        return bool(keys & pool_assets)

    if tx_type in DEX_TX_TYPES:
        gets = amount_to_asset_key(tx.get("TakerGets"))
        pays = amount_to_asset_key(tx.get("TakerPays"))
        return gets in pool_assets and pays in pool_assets

    return False


def xrpl_close_to_unix(close_time: int) -> int:
    return XRPL_EPOCH_OFFSET + int(close_time)


def run_snapshot(config_path: Path, out_path: Path) -> None:
    cfg = read_json(config_path)
    rpc = XRPLRPC(cfg["rpc_url"])

    top_n = int(cfg.get("top_lp_holders_per_pool", 10))
    target_pool_count = int(cfg.get("target_pool_count", 25))
    strict = bool(cfg.get("strict_pool_count", True))

    snapshot_ledger = rpc.latest_validated_ledger()
    mode = str(cfg.get("pool_selection_mode", "configured")).lower()
    if mode == "discover":
        configured_pools = discover_pools_from_ledger(rpc, snapshot_ledger, target_pool_count)
    else:
        configured_pools = cfg.get("pools", [])
        if len(configured_pools) == 0:
            raise RuntimeError("No pools configured. Add entries under 'pools' in config.")

    if strict and len(configured_pools) != target_pool_count:
        raise RuntimeError(
            f"Selected pools ({len(configured_pools)}) must equal target_pool_count ({target_pool_count})"
        )

    snapshot: Dict[str, Any] = {
        "generated_at_unix": int(time.time()),
        "snapshot_ledger_index": snapshot_ledger,
        "pool_selection_mode": mode,
        "target_pool_count": target_pool_count,
        "top_lp_holders_per_pool": top_n,
        "pools": [],
    }

    for p in configured_pools:
        asset_0 = parse_asset(p["asset_0"])
        asset_1 = parse_asset(p["asset_1"])
        amm_account_hint = p.get("amm_account")
        if amm_account_hint:
            amm_result = rpc.amm_info_by_account(amm_account_hint, snapshot_ledger)
        else:
            amm_result = rpc.amm_info(asset_0, asset_1, snapshot_ledger)
        amm = amm_result.get("amm", {})
        if not amm:
            raise RuntimeError(f"amm_info returned no AMM object for pool {p.get('name', '<unnamed>')}")

        amm_account = amm.get("account")
        lp_token = amm.get("lp_token", {})
        lp_currency = lp_token.get("currency", "")
        lp_issuer = lp_token.get("issuer", amm_account)

        holders: List[Dict[str, Any]] = []
        if amm_account and lp_currency:
            lines = rpc.account_lines(amm_account, snapshot_ledger)
            for line in lines:
                if line.get("currency") != lp_currency:
                    continue
                if lp_issuer and line.get("account") == lp_issuer:
                    continue
                bal_raw = line.get("balance", "0")
                try:
                    bal = abs(float(bal_raw))
                except ValueError:
                    continue
                if bal <= 0:
                    continue
                holders.append(
                    {
                        "account": line.get("account"),
                        "balance": bal_raw,
                        "balance_abs": bal,
                    }
                )
            holders.sort(key=lambda h: h["balance_abs"], reverse=True)
            holders = holders[:top_n]
            for h in holders:
                h.pop("balance_abs", None)

        snapshot["pools"].append(
            {
                "name": p.get("name", f"{asset_0.key()}-{asset_1.key()}"),
                "asset_0": {"kind": asset_0.kind, "currency": asset_0.currency, "issuer": asset_0.issuer},
                "asset_1": {"kind": asset_1.kind, "currency": asset_1.currency, "issuer": asset_1.issuer},
                "amm_account": amm_account,
                "trading_fee": amm.get("trading_fee"),
                "amount": amm.get("amount"),
                "amount2": amm.get("amount2"),
                "lp_token": lp_token,
                "top_lp_holders": holders,
            }
        )

    write_json(out_path, snapshot)
    print(f"Wrote snapshot: {out_path}")


def run_replay(config_path: Path, snapshot_path: Path, out_path: Path) -> None:
    cfg = read_json(config_path)
    rpc = XRPLRPC(cfg["rpc_url"])
    snapshot = read_json(snapshot_path)

    replay_window_secs = int(cfg.get("replay_window_secs", 3600))
    latest = rpc.latest_validated_ledger()

    pool_asset_keys = set()
    for p in snapshot.get("pools", []):
        a0 = parse_asset(p["asset_0"])
        a1 = parse_asset(p["asset_1"])
        pool_asset_keys.add(a0.key())
        pool_asset_keys.add(a1.key())

    now_unix = int(time.time())
    cutoff = now_unix - replay_window_secs

    replay_txs: List[Dict[str, Any]] = []
    current = latest
    scanned_ledgers = 0

    while current > 0:
        result = rpc.ledger_with_transactions(current)
        ledger = result.get("ledger", {})
        close_time = int(ledger.get("close_time", 0))
        close_unix = xrpl_close_to_unix(close_time)
        scanned_ledgers += 1

        if close_unix < cutoff:
            break

        txs = ledger.get("transactions", [])
        for raw in txs:
            tx = raw.get("tx_json", raw)
            tx_type = tx.get("TransactionType")
            if tx_type not in AMM_TX_TYPES and tx_type not in DEX_TX_TYPES:
                continue
            if not tx_touches_pool(tx, pool_asset_keys):
                continue
            replay_txs.append(
                {
                    "ledger_index": current,
                    "ledger_close_unix": close_unix,
                    "hash": tx.get("hash"),
                    "tx_type": tx_type,
                    "account": tx.get("Account"),
                    "tx_json": tx,
                }
            )
        current -= 1

    replay_txs.sort(key=lambda t: (t["ledger_index"], t.get("hash") or ""))
    replay = {
        "generated_at_unix": now_unix,
        "latest_ledger_index": latest,
        "replay_window_secs": replay_window_secs,
        "cutoff_unix": cutoff,
        "scanned_ledgers": scanned_ledgers,
        "transactions": replay_txs,
    }
    write_json(out_path, replay)
    print(f"Wrote replay: {out_path}")


def _safe_amount_float(amount: Any) -> float:
    if isinstance(amount, str):
        try:
            return float(amount)
        except ValueError:
            return 0.0
    if isinstance(amount, dict):
        try:
            return float(amount.get("value", "0"))
        except ValueError:
            return 0.0
    return 0.0


def run_seed_spec(config_path: Path, snapshot_path: Path, replay_path: Path, out_path: Path) -> None:
    cfg = read_json(config_path)
    snapshot = read_json(snapshot_path)
    replay = read_json(replay_path)

    pools_out = []
    for p in snapshot.get("pools", []):
        reserve_0 = _safe_amount_float(p.get("amount"))
        reserve_1 = _safe_amount_float(p.get("amount2"))
        sqrt_price_q64_64 = 0
        if reserve_0 > 0 and reserve_1 > 0:
            sqrt_price_q64_64 = int(math.sqrt(reserve_1 / reserve_0) * (2**64))
        pools_out.append(
            {
                "name": p["name"],
                "asset_0": p["asset_0"],
                "asset_1": p["asset_1"],
                "amm_account": p.get("amm_account"),
                "trading_fee": p.get("trading_fee"),
                "reserve_0": p.get("amount"),
                "reserve_1": p.get("amount2"),
                "sqrt_price_q64_64": str(sqrt_price_q64_64),
                "top_lp_holders": p.get("top_lp_holders", []),
            }
        )

    spec = {
        "version": "0.1",
        "generated_at_unix": int(time.time()),
        "profile": {
            "target_pool_count": int(cfg.get("target_pool_count", 25)),
            "top_lp_holders_per_pool": int(cfg.get("top_lp_holders_per_pool", 10)),
            "replay_window_secs": int(cfg.get("replay_window_secs", 3600)),
        },
        "snapshot": {
            "ledger_index": snapshot.get("snapshot_ledger_index"),
            "pool_count": len(snapshot.get("pools", [])),
        },
        "replay": {
            "latest_ledger_index": replay.get("latest_ledger_index"),
            "replay_window_secs": replay.get("replay_window_secs"),
            "transaction_count": len(replay.get("transactions", [])),
        },
        "pools": pools_out,
        "replay_transactions": replay.get("transactions", []),
    }
    write_json(out_path, spec)
    print(f"Wrote seed spec: {out_path}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="XRPL-in-Time state extraction pipeline")
    sub = parser.add_subparsers(dest="command", required=True)

    p_snapshot = sub.add_parser("snapshot", help="Extract pool + LP holder snapshot")
    p_snapshot.add_argument("--config", required=True, type=Path)
    p_snapshot.add_argument("--out", required=True, type=Path)

    p_replay = sub.add_parser("replay", help="Extract replay transactions for configurable window")
    p_replay.add_argument("--config", required=True, type=Path)
    p_replay.add_argument("--snapshot", required=True, type=Path)
    p_replay.add_argument("--out", required=True, type=Path)

    p_seed = sub.add_parser("seed-spec", help="Build deterministic seed spec")
    p_seed.add_argument("--config", required=True, type=Path)
    p_seed.add_argument("--snapshot", required=True, type=Path)
    p_seed.add_argument("--replay", required=True, type=Path)
    p_seed.add_argument("--out", required=True, type=Path)
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "snapshot":
        run_snapshot(args.config, args.out)
    elif args.command == "replay":
        run_replay(args.config, args.snapshot, args.out)
    elif args.command == "seed-spec":
        run_seed_spec(args.config, args.snapshot, args.replay, args.out)
    else:
        raise RuntimeError(f"Unknown command: {args.command}")


if __name__ == "__main__":
    main()
