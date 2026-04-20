"""Build a ``PoolState`` snapshot at a historical block via the generic RpcClient.

Used by the on-chain swap-replay validation harness. Needs archive access
(``eth_call`` at the target block minus one). Reuses the tick-bitmap scan from
Module 1 so we don't have duplicate logic.
"""

from __future__ import annotations

from eth_abi import decode as abi_decode

from fabdl.core.constants import FEE_TIER, POOL_ADDRESS
from fabdl.extract.liquidity_snapshots import (
    _enumerate_initialized_ticks,
    _fetch_tick_details,
)
from fabdl.extract.slot0 import _fetch_slot0
from fabdl.io.rpc import RpcClient
from fabdl.simulate.swap import PoolState


def load_pool_state_at_block(rpc: RpcClient, block: int) -> PoolState:
    slot0 = _fetch_slot0(rpc, block)
    liquidity_raw = rpc.call(POOL_ADDRESS, bytes.fromhex("1a686502"), block=block)  # liquidity()
    (active_liquidity,) = abi_decode(["uint128"], liquidity_raw)
    ticks = _enumerate_initialized_ticks(rpc, block)
    details = _fetch_tick_details(rpc, block, ticks)
    return PoolState(
        sqrt_price_x96=slot0["sqrt_price_x96"],
        tick=slot0["tick"],
        liquidity=active_liquidity,
        fee_pips=FEE_TIER,
        initialized_ticks=sorted(ticks),
        liquidity_net={d.tick: d.liquidity_net for d in details},
    )
