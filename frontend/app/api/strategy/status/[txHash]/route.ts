import { NextResponse } from 'next/server'

export async function GET(
  request: Request,
  { params }: { params: Promise<{ txHash: string }> }
) {
  try {
    const { txHash } = await params

    if (!txHash) {
      return NextResponse.json(
        { error: 'Missing transaction hash' },
        { status: 400 }
      )
    }

    // In production, this would query the XRPL for actual transaction status
    // For demo, we return a mock confirmed status

    return NextResponse.json({
      tx_hash: txHash,
      status: 'confirmed',
      confirmations: 3,
      block_number: Math.floor(Math.random() * 1000000) + 80000000,
      timestamp: new Date().toISOString(),
      explorer_url: `https://testnet.xrpl.org/transactions/${txHash}`
    })
  } catch (error) {
    console.error('Error fetching transaction status:', error)
    return NextResponse.json(
      { error: 'Failed to fetch transaction status' },
      { status: 500 }
    )
  }
}
