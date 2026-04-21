"""`fabdl` root CLI — wraps per-module subcommands."""

from __future__ import annotations

import logging
from datetime import date, datetime
from pathlib import Path

import typer
from dotenv import load_dotenv

from fabdl.io.rpc import RpcClient, RpcConfig

app = typer.Typer(no_args_is_help=True, add_completion=False)
extract_app = typer.Typer(no_args_is_help=True, help="Module 1 — on-chain data extraction")
lp_app = typer.Typer(no_args_is_help=True, help="Module 4 — LP position analytics")
app.add_typer(extract_app, name="extract")
app.add_typer(lp_app, name="lp")


def _setup() -> tuple[RpcClient, Path]:
    load_dotenv()
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s")
    data_dir = Path("data")
    rpc = RpcClient(RpcConfig.from_env(), data_dir=data_dir)
    return rpc, data_dir


def _parse_date(s: str) -> date:
    return datetime.strptime(s, "%Y-%m-%d").date()


@extract_app.command("swaps")
def cmd_swaps(
    from_date: str = typer.Option(None, "--from"),
    to_date: str = typer.Option(None, "--to"),
    compact_output: bool = typer.Option(True, "--compact/--no-compact"),
    from_block: int = typer.Option(None, "--from-block", help="Override start block (skips date→block lookup)"),
    to_block: int = typer.Option(None, "--to-block", help="Override end block (skips date→block lookup)"),
    parts_subdir: str = typer.Option(None, "--parts-dir", help="Parts dir name under data/raw/ (default: swap_events_parts)"),
    checkpoint_name: str = typer.Option(None, "--checkpoint", help="Checkpoint filename under data/checkpoints/ (default: swap_events.json)"),
) -> None:
    """Fetch Swap events for a date range."""
    from fabdl.extract.swaps import compact_swaps, extract_swaps
    from fabdl.io.block_index import BlockIndex

    rpc, data_dir = _setup()
    if from_block is None or to_block is None:
        if from_date is None or to_date is None:
            raise typer.BadParameter("--from and --to are required when --from-block/--to-block are not both supplied")
        bi = BlockIndex(rpc, cache_path=data_dir / "checkpoints" / "block_index.sqlite")
        if from_block is None:
            from_block = bi.block_at_timestamp(int(datetime.combine(_parse_date(from_date), datetime.min.time()).timestamp()))
        if to_block is None:
            to_block = bi.block_at_timestamp(int(datetime.combine(_parse_date(to_date), datetime.max.time()).timestamp()))
    parts_dir = (data_dir / "raw" / parts_subdir) if parts_subdir else None
    ckpt_path = (data_dir / "checkpoints" / checkpoint_name) if checkpoint_name else None
    n = extract_swaps(rpc, from_block, to_block, out_dir=data_dir, parts_dir=parts_dir, checkpoint_path=ckpt_path)
    typer.echo(f"fetched {n} swap events over blocks {from_block}..{to_block}")
    if compact_output:
        rows = compact_swaps(data_dir)
        typer.echo(f"compacted to swap_events.parquet ({rows} rows)")


@extract_app.command("mints-burns")
def cmd_mints_burns(
    from_date: str = typer.Option(..., "--from"),
    to_date: str = typer.Option(..., "--to"),
    compact_output: bool = typer.Option(True, "--compact/--no-compact"),
) -> None:
    from fabdl.extract.mints_burns import compact_mints_burns, extract_mints_burns
    from fabdl.io.block_index import BlockIndex

    rpc, data_dir = _setup()
    bi = BlockIndex(rpc, cache_path=data_dir / "checkpoints" / "block_index.sqlite")
    fb = bi.block_at_timestamp(int(datetime.combine(_parse_date(from_date), datetime.min.time()).timestamp()))
    tb = bi.block_at_timestamp(int(datetime.combine(_parse_date(to_date), datetime.max.time()).timestamp()))
    n = extract_mints_burns(rpc, fb, tb, out_dir=data_dir)
    typer.echo(f"fetched {n} mint/burn events over blocks {fb}..{tb}")
    if compact_output:
        rows = compact_mints_burns(data_dir)
        typer.echo(f"compacted to mint_burn_events.parquet ({rows} rows)")


@extract_app.command("slot0")
def cmd_slot0(
    from_date: str = typer.Option(..., "--from"),
    to_date: str = typer.Option(..., "--to"),
    compact_output: bool = typer.Option(True, "--compact/--no-compact"),
) -> None:
    from fabdl.extract.slot0 import compact_slot0, extract_slot0_daily

    rpc, data_dir = _setup()
    n = extract_slot0_daily(rpc, _parse_date(from_date), _parse_date(to_date), out_dir=data_dir)
    typer.echo(f"wrote {n} daily slot0 snapshots")
    if compact_output:
        rows = compact_slot0(data_dir)
        typer.echo(f"compacted to slot0_snapshots.parquet ({rows} rows)")


@extract_app.command("liquidity-snapshots")
def cmd_liquidity_snapshots(
    from_date: str = typer.Option(..., "--from"),
    to_date: str = typer.Option(..., "--to"),
    compact_output: bool = typer.Option(True, "--compact/--no-compact"),
    force_path_b: bool = typer.Option(
        False, "--force-path-b", help="Reconstruct from Mint/Burn events instead of archive eth_call."
    ),
) -> None:
    from fabdl.extract.liquidity_snapshots import (
        compact_liquidity_snapshots,
        extract_liquidity_snapshots_daily,
    )

    rpc, data_dir = _setup()
    n = extract_liquidity_snapshots_daily(
        rpc,
        _parse_date(from_date),
        _parse_date(to_date),
        out_dir=data_dir,
        force_path_b=force_path_b,
    )
    typer.echo(f"wrote {n} liquidity-snapshot rows")
    if compact_output:
        rows = compact_liquidity_snapshots(data_dir)
        typer.echo(f"compacted to liquidity_snapshots.parquet ({rows} rows)")


@lp_app.command("analyze")
def cmd_lp_analyze(
    tick_lower: int = typer.Option(..., help="Lower tick of the LP range"),
    tick_upper: int = typer.Option(..., help="Upper tick of the LP range"),
    liquidity: int = typer.Option(..., help="Position virtual liquidity (uint128)"),
    from_date: str = typer.Option(..., "--from", help="Start date YYYY-MM-DD (inclusive)"),
    to_date: str = typer.Option(..., "--to", help="End date YYYY-MM-DD (inclusive)"),
    compact_output: bool = typer.Option(True, "--compact/--no-compact"),
) -> None:
    """Track a synthetic LP position: fees, IL, and PnL vs HODL.

    Reads ``data/processed/slot0_snapshots.parquet`` and
    ``data/processed/liquidity_snapshots.parquet``.  Writes daily snapshots to
    ``data/processed/lp_analytics.parquet`` (appends if position_id differs).
    """
    from fabdl.io.parquet import append_rows, compact, lp_analytics_schema
    from fabdl.lp.analytics import position_stats, snapshots_to_rows, track_position
    from fabdl.lp.position import Position

    _, data_dir = _setup()
    pos = Position(tick_lower=tick_lower, tick_upper=tick_upper, liquidity=liquidity)

    snapshots = track_position(pos, data_dir, _parse_date(from_date), _parse_date(to_date))
    typer.echo(f"computed {len(snapshots)} daily snapshots for range [{tick_lower}, {tick_upper}]")

    stats = position_stats(snapshots, pos)
    typer.echo(
        f"entry ${stats.entry_value_usd:,.2f}  |  "
        f"fees ${stats.total_fees_usd:,.2f}  |  "
        f"IL {stats.il_ratio_final:.2%}  |  "
        f"PnL vs HODL ${stats.total_pnl_usd:,.2f} ({stats.total_pnl_pct:.2f}%)  |  "
        f"fee APR {stats.fee_apr:.2%}"
    )

    rows = snapshots_to_rows(snapshots, pos)
    schema = lp_analytics_schema()
    parts_dir = data_dir / "raw" / "lp_analytics_parts"
    append_rows(parts_dir, rows, schema)

    if compact_output:
        out = data_dir / "processed" / "lp_analytics.parquet"
        n = compact(parts_dir, out, schema)
        typer.echo(f"compacted to lp_analytics.parquet ({n} rows)")


if __name__ == "__main__":
    app()
