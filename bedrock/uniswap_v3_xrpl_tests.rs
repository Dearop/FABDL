#[cfg(test)]
mod tests {
    use crate::uniswap_v3_xrpl_contract_skeleton::{AccountId, ContractError, UniswapV3XrplContract};
    use crate::uniswap_v3_xrpl_execution_adapter::{DualPathAdapter, SwapRequest};

    fn owner() -> AccountId {
        [7u8; 20]
    }

    #[test]
    fn invariant_tick_range_must_be_ordered() {
        let mut c = UniswapV3XrplContract::new(owner(), 10);
        let result = c.mint(owner(), 10, 10, 1000);
        assert!(matches!(result, Err(ContractError::InvalidTickRange)));
    }

    #[test]
    fn invariant_tick_spacing_enforced() {
        let mut c = UniswapV3XrplContract::new(owner(), 10);
        let result = c.mint(owner(), 11, 21, 1000);
        assert!(matches!(result, Err(ContractError::TickSpacingViolation)));
    }

    #[test]
    fn guard_pause_blocks_state_mutation() {
        let mut c = UniswapV3XrplContract::new(owner(), 10);
        c.set_pause(owner(), 1).unwrap();
        let result = c.mint(owner(), -20, 20, 1000);
        assert!(matches!(result, Err(ContractError::Paused)));
    }

    #[test]
    fn slippage_guard_rejects_low_min_out() {
        let mut c = UniswapV3XrplContract::new(owner(), 10);
        let result = c.swap_exact_in(owner(), 1000, 999, 1, 1u128 << 64);
        assert!(result.is_err());
    }

    #[test]
    fn adapter_falls_back_to_direct_xrpl() {
        let adapter = DualPathAdapter {
            prefer_bedrock: true,
            bedrock_available: false,
            xrpl_available: true,
        };
        let req = SwapRequest {
            amount_in: 500,
            min_amount_out: 490,
            zero_for_one: true,
        };
        let receipt = adapter.execute_with_fallback(&req).unwrap();
        assert_eq!(receipt.amount_out, 498);
    }
}
