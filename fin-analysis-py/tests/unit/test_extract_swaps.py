"""Swap decoder correctness — synthesized logs with known values."""

from __future__ import annotations

from eth_abi import encode as abi_encode

from fabdl.core.constants import Q96
from fabdl.core.events import SWAP_TOPIC0
from fabdl.extract.swaps import _decode_swap, _transform


def _fake_swap_log(
    *,
    block: int,
    tx: bytes,
    log_index: int,
    sender: bytes,
    recipient: bytes,
    amount0: int,
    amount1: int,
    sqrt_price_x96: int,
    liquidity: int,
    tick: int,
) -> dict:
    data = abi_encode(
        ["int256", "int256", "uint160", "uint128", "int24"],
        [amount0, amount1, sqrt_price_x96, liquidity, tick],
    )
    return {
        "blockNumber": hex(block),
        "transactionHash": "0x" + tx.hex(),
        "logIndex": hex(log_index),
        "topics": [
            SWAP_TOPIC0,
            "0x" + sender.rjust(32, b"\x00").hex(),
            "0x" + recipient.rjust(32, b"\x00").hex(),
        ],
        "data": "0x" + data.hex(),
    }


def test_decode_swap_roundtrip():
    lg = _fake_swap_log(
        block=15_000_000,
        tx=b"\xab" * 32,
        log_index=7,
        sender=b"\x11" * 20,
        recipient=b"\x22" * 20,
        amount0=-1_000_000_000,      # pool sent out 1000 USDC (amount0_raw with 6 dec)
        amount1=500_000_000_000_000,  # pool received 0.0005 WETH
        sqrt_price_x96=Q96,            # implies raw price_token1_per_token0 == 1
        liquidity=10**20,
        tick=0,
    )
    ev = _decode_swap(lg)
    assert ev["amount0"] == -1_000_000_000
    assert ev["amount1"] == 500_000_000_000_000
    assert ev["sqrt_price_x96"] == Q96
    assert ev["liquidity"] == 10**20
    assert ev["tick"] == 0


def test_transform_direction_and_notional():
    # amount0 > 0 means user paid USDC → zeroForOne.
    lg = _fake_swap_log(
        block=18_000_000,
        tx=b"\xcd" * 32,
        log_index=3,
        sender=b"\x33" * 20,
        recipient=b"\x44" * 20,
        amount0=2_000_000_000,          # +2000 USDC paid in
        amount1=-1_000_000_000_000_000,  # -0.001 WETH received
        sqrt_price_x96=Q96,
        liquidity=10**20,
        tick=0,
    )
    row = _transform(lg, timestamp=1_700_000_000)
    assert row["direction"] == "zeroForOne"
    assert row["usd_notional"] == 2000.0
    assert row["amount0_usdc"] == 2000.0
    assert row["amount1_weth"] == -0.001
    # sqrtPriceX96 == Q96 → raw token1/token0 == 1; human WETH per USDC = 1e-12.
    assert row["price_weth_per_usdc"] == 1e-12
    assert row["date"] == "2023-11-14"


def test_one_for_zero_direction():
    lg = _fake_swap_log(
        block=18_000_000,
        tx=b"\xcd" * 32,
        log_index=3,
        sender=b"\x33" * 20,
        recipient=b"\x44" * 20,
        amount0=-1_000_000_000,
        amount1=500_000_000_000_000,
        sqrt_price_x96=Q96,
        liquidity=10**20,
        tick=0,
    )
    row = _transform(lg, timestamp=1_700_000_000)
    assert row["direction"] == "oneForZero"
