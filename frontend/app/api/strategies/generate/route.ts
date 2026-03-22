import { NextResponse } from 'next/server'
import type { Strategy } from '@/lib/types'

// Mock strategy generation - fallback when real LLM backend is unavailable
function generateStrategies(query: string, walletId: string): Strategy[] {
  const strategies: Strategy[] = [
    {
      id: 'option_a',
      title: 'Conservative: Swap to Stablecoin',
      description: 'Reduce directional exposure by swapping half your XRP to USD. Preserves capital in volatile markets.',
      risk_score: 2,
      projected_return_7d: {
        expected: '$5',
        best_case: '$15',
        worst_case: '-$3'
      },
      trade_actions: [
        { action: 'swap', asset_in: 'XRP', asset_out: 'USD', amount: 50, estimated_slippage: 0.3, pool: 'XRP/USD', deposit_mode: null, amount2: null, interest_rate: null, term_days: null }
      ],
      pros: ['Low risk exposure', 'Capital preservation'],
      cons: ['Limited upside', 'Transaction fees apply']
    },
    {
      id: 'option_b',
      title: 'Yield: Two-Asset AMM Deposit',
      description: 'Deposit XRP and USD proportionally into the XRP/USD AMM pool to earn trading fees. No deposit fee for proportional deposits.',
      risk_score: 5,
      projected_return_7d: {
        expected: '$12',
        best_case: '$30',
        worst_case: '-$8'
      },
      trade_actions: [
        { action: 'deposit', asset_in: 'XRP', asset_out: 'USD', amount: 40, estimated_slippage: 0.0, pool: 'XRP/USD', deposit_mode: 'two_asset', amount2: 100, interest_rate: null, term_days: null }
      ],
      pros: ['Earn trading fees', 'No deposit fee (proportional)'],
      cons: ['Impermanent loss risk', 'Capital locked in pool']
    },
    {
      id: 'option_c',
      title: 'Do Nothing: Hold Position',
      description: 'Keep current holdings unchanged. Monitor market conditions before committing capital.',
      risk_score: 2,
      projected_return_7d: {
        expected: '$0',
        best_case: '$0',
        worst_case: '$0'
      },
      trade_actions: [],
      pros: ['Zero transaction cost', 'No action needed'],
      cons: ['No yield earned', 'Exposed to XRP price movement']
    }
  ]

  return strategies
}

export async function POST(request: Request) {
  try {
    const body = await request.json()
    const { user_query, wallet_id } = body

    if (!user_query || !wallet_id) {
      return NextResponse.json(
        { error: 'Missing required fields: user_query and wallet_id' },
        { status: 400 }
      )
    }

    // Simulate LLM processing time
    await new Promise(resolve => setTimeout(resolve, 1500))

    const strategies = generateStrategies(user_query, wallet_id)

    return NextResponse.json({ 
      success: true, 
      strategies,
      query: user_query,
      generated_at: new Date().toISOString()
    })
  } catch (error) {
    console.error('Error generating strategies:', error)
    return NextResponse.json(
      { error: 'Failed to generate strategies' },
      { status: 500 }
    )
  }
}
