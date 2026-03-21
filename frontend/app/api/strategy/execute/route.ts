import { NextResponse } from 'next/server'
import type { TradeAction } from '@/lib/types'

function buildExecutionSummary(strategy: { trade_actions?: TradeAction[] }) {
  const actions = strategy?.trade_actions || []
  const lines: string[] = []

  if (actions.length === 0) {
    lines.push('No trades executed — position held unchanged.')
  } else {
    for (const a of actions) {
      if (a.action === 'swap') {
        lines.push(`Swapped ${a.amount} ${a.asset_in} → ${a.asset_out}${a.pool ? ` via ${a.pool} pool` : ''}`)
      } else if (a.action === 'deposit') {
        if (a.deposit_mode === 'two_asset' && a.amount2) {
          lines.push(`Two-asset deposit: ${a.amount} ${a.asset_in} + ${a.amount2} ${a.asset_out} into ${a.pool || 'AMM'} pool`)
        } else {
          lines.push(`Single-asset deposit: ${a.amount} ${a.asset_in} into ${a.pool || 'AMM'} pool`)
        }
      } else if (a.action === 'withdraw') {
        lines.push(`Withdrew ${a.amount} ${a.asset_in} from ${a.pool || 'AMM'} pool`)
      } else if (a.action === 'lend') {
        lines.push(`Supplied ${a.amount} ${a.asset_in} to lending vault${a.interest_rate ? ` at ${a.interest_rate}% APR` : ''}${a.term_days ? ` for ${a.term_days} days` : ''}`)
      } else if (a.action === 'borrow') {
        lines.push(`Borrowed ${a.amount} ${a.asset_out} (collateral: ${a.amount} ${a.asset_in})${a.interest_rate ? ` at ${a.interest_rate}% APR` : ''}`)
      }
    }
  }

  const hasDeposits = actions.some(a => a.action === 'deposit')
  const hasLending = actions.some(a => a.action === 'lend')

  return {
    simulated: true,
    summary_lines: lines,
    il_estimate: hasDeposits ? 'Estimated IL at ±10% price move: ~-0.5%' : undefined,
    fee_estimate: hasDeposits ? 'Estimated Fee APR: 5-15% (depends on pool volume)' : hasLending ? `Lending yield: ${actions.find(a => a.action === 'lend')?.interest_rate || 5}% APR` : undefined,
    net_cost: 'Est. network fee: 0.000012 XRP per transaction',
  }
}

export async function POST(request: Request) {
  try {
    const body = await request.json()
    const { strategy_id, wallet_id, strategy } = body

    if (!strategy_id || !wallet_id) {
      return NextResponse.json(
        { error: 'Missing required fields: strategy_id and wallet_id' },
        { status: 400 }
      )
    }

    // Simulate transaction processing time
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Generate simulated transaction hash
    const txHash = `${Math.random().toString(36).substring(2, 10).toUpperCase()}${Date.now().toString(36).toUpperCase()}`

    return NextResponse.json({
      success: true,
      tx_hash: txHash,
      strategy_id,
      wallet_id,
      status: 'confirmed',
      confirmed_at: new Date().toISOString(),
      execution_summary: buildExecutionSummary(strategy || {}),
    })
  } catch (error) {
    console.error('Error executing strategy:', error)
    return NextResponse.json(
      { error: 'Failed to execute strategy' },
      { status: 500 }
    )
  }
}
