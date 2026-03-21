## IL Calculation (per position, at a hypothetical price P)

amount0_at_P, amount1_at_P = burn(lower_tick, upper_tick, liquidity) simulated at price P
hodl_value = amount0_at_entry * price_P + amount1_at_entry (what you'd have if you just held)
position_value_at_P = amount0_at_P * price_P + amount1_at_P
il_ratio = (position_value_at_P - hodl_value) / hodl_value (negative = loss)

## Fee Growth Accounting (V3 per-position)

fee_below_lower = if current_tick >= lower_tick then fee_growth_outside_lower else fee_growth_global - fee_growth_outside_lower
fee_above_upper = if current_tick < upper_tick then fee_growth_outside_upper else fee_growth_global - fee_growth_outside_upper
fee_growth_inside = fee_growth_global - fee_below_lower - fee_above_upper

## Fees Earned (per position, per token)

fees_earned_token0 = position_liquidity * (fee_growth_inside_0_now - fee_growth_inside_0_last) / 2^128
fees_earned_token1 = position_liquidity * (fee_growth_inside_1_now - fee_growth_inside_1_last) / 2^128

## Fee APR (annualised)

fees_earned_usd = fees_earned_token0 * price_token0_usd + fees_earned_token1 * price_token1_usd
position_value_usd = amount0_held * price_token0_usd + amount1_held * price_token1_usd
fee_apr = (fees_earned_usd / position_value_usd) * (seconds_per_year / replay_window_secs)

## Break-Even (two prices)

break_even_upper = smallest price P above current price where il_ratio(P) + fee_apr_as_fraction = 0
break_even_lower = largest price P below current price where il_ratio(P) + fee_apr_as_fraction = 0
Equivalently: the two prices where abs(il_value) = fees_earned_usd

## Swap Volume Replay (fee_growth_global accumulation)

For each historical swap from account_tx within replay_window_secs:
fee_amount = swap_amount_in * fee_bps / 10_000
fee_growth_global_delta = fee_amount * 2^128 / active_liquidity
fee_growth_global += fee_growth_global_delta
(tick crossings update active_liquidity and flip fee_growth_outside accumulators per the V3 swap loop)