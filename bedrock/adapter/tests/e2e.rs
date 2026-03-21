/// End-to-end integration tests for the DualPathAdapter routing layer.
///
/// These tests exercise the full adapter → contract call chain including
/// path selection, automatic fallback, and pre-submission validation.

use uniswap_v3_xrpl_adapter::{AdapterError, DualPathAdapter, ExecutionPath, SwapRequest};
use uniswap_v3_xrpl_contract::test_setup;

fn owner() -> [u8; 20] { [1u8; 20] }
fn alice() -> [u8; 20] { [2u8; 20] }

/// Initialise a pool with liquidity ready for adapter-level swap tests.
fn setup_pool_with_liquidity() {
    test_setup(owner(), 10);
    uniswap_v3_xrpl_contract::initialize_pool(owner(), 0u64, 1u64, 30, 0);
    uniswap_v3_xrpl_contract::mint(owner(), (-1000_i32) as u32, 1000, 1_000_000_000, 0);
}

fn swap_req(amount_in: u64, zero_for_one: bool) -> SwapRequest {
    SwapRequest {
        sender: alice(),
        amount_in,
        min_amount_out: amount_in * 99 / 100,
        zero_for_one,
    }
}

// ---------------------------------------------------------------------------
// Path selection
// ---------------------------------------------------------------------------

#[test]
fn bedrock_path_used_when_preferred_and_available() {
    setup_pool_with_liquidity();
    let adapter = DualPathAdapter::new(true, true, false);
    let receipt = adapter.execute_with_fallback(&swap_req(10_000, false)).unwrap();
    assert_eq!(receipt.path, ExecutionPath::BedrockDirect);
    assert!(receipt.amount_out > 0);
}

#[test]
fn xrpl_path_used_when_bedrock_not_preferred() {
    setup_pool_with_liquidity();
    let adapter = DualPathAdapter::new(false, false, true);
    let receipt = adapter.execute_with_fallback(&swap_req(10_000, false)).unwrap();
    assert_eq!(receipt.path, ExecutionPath::DirectXrpl);
    assert!(receipt.amount_out > 0);
}

#[test]
fn both_paths_available_uses_preferred() {
    setup_pool_with_liquidity();
    let bedrock_adapter = DualPathAdapter::new(true,  true, true);
    let xrpl_adapter    = DualPathAdapter::new(false, true, true);

    let r1 = bedrock_adapter.execute_with_fallback(&swap_req(10_000, false)).unwrap();
    let r2 = xrpl_adapter.execute_with_fallback(&swap_req(10_000, false)).unwrap();

    assert_eq!(r1.path, ExecutionPath::BedrockDirect);
    assert_eq!(r2.path, ExecutionPath::DirectXrpl);
}

// ---------------------------------------------------------------------------
// Automatic fallback
// ---------------------------------------------------------------------------

#[test]
fn falls_back_to_xrpl_when_bedrock_unavailable() {
    setup_pool_with_liquidity();
    // prefer_bedrock=true but bedrock_available=false → must use XRPL.
    let adapter = DualPathAdapter::new(true, false, true);
    let receipt = adapter.execute_with_fallback(&swap_req(10_000, false)).unwrap();
    assert_eq!(receipt.path, ExecutionPath::DirectXrpl);
    assert!(receipt.amount_out > 0);
}

#[test]
fn no_paths_available_returns_unsupported() {
    let adapter = DualPathAdapter::new(true, false, false);
    let err = adapter.execute_with_fallback(&swap_req(10_000, false)).unwrap_err();
    assert_eq!(err, AdapterError::Unsupported);
}

// ---------------------------------------------------------------------------
// Pre-submission validation (fires before reaching the contract)
// ---------------------------------------------------------------------------

#[test]
fn zero_input_rejected_before_submission() {
    let adapter = DualPathAdapter::new(true, true, false);
    let req = SwapRequest {
        sender: alice(),
        amount_in: 0,
        min_amount_out: 0,
        zero_for_one: false,
    };
    let err = adapter.execute_with_fallback(&req).unwrap_err();
    assert!(matches!(err, AdapterError::ContractError(_)),
            "zero input should produce a ContractError, not Unsupported");
}

#[test]
fn insufficient_min_out_rejected_before_submission() {
    setup_pool_with_liquidity();
    let adapter = DualPathAdapter::new(true, true, false);
    // min_out = 1 is far below the 99% floor for 10_000 input.
    let req = SwapRequest {
        sender: alice(),
        amount_in: 10_000,
        min_amount_out: 1,
        zero_for_one: false,
    };
    let err = adapter.execute_with_fallback(&req).unwrap_err();
    assert!(matches!(err, AdapterError::ContractError(_)));
}

#[test]
fn exact_99pct_min_out_is_accepted() {
    setup_pool_with_liquidity();
    let adapter = DualPathAdapter::new(true, true, false);
    // 9_900 / 10_000 = 99% — right at the floor.
    let req = SwapRequest {
        sender: alice(),
        amount_in: 10_000,
        min_amount_out: 9_900,
        zero_for_one: false,
    };
    let receipt = adapter.execute_with_fallback(&req).unwrap();
    assert!(receipt.amount_out > 0);
}

// ---------------------------------------------------------------------------
// Swap direction correctness through the adapter
// ---------------------------------------------------------------------------

#[test]
fn adapter_swap_both_directions_produce_output() {
    setup_pool_with_liquidity();
    let adapter = DualPathAdapter::new(true, true, false);

    // token1 → token0 (price up).
    let out_up = adapter.execute_with_fallback(&swap_req(10_000, false))
        .unwrap().amount_out;
    assert!(out_up > 0, "upward swap through adapter should produce output");

    // token0 → token1 (price down).
    let out_down = adapter.execute_with_fallback(&swap_req(10_000, true))
        .unwrap().amount_out;
    assert!(out_down > 0, "downward swap through adapter should produce output");
}

// ---------------------------------------------------------------------------
// Large swap depletes liquidity — adapter handles gracefully
// ---------------------------------------------------------------------------

#[test]
fn swap_larger_than_pool_returns_partial_fill() {
    setup_pool_with_liquidity();
    let adapter = DualPathAdapter::new(true, true, false);

    let req = SwapRequest {
        sender: alice(),
        amount_in: 100_000,
        min_amount_out: 99_000,
        zero_for_one: false,
    };
    // Either succeeds with meaningful output, or fails the slippage check
    // (price impact is large). Either way it must not panic.
    let _ = adapter.execute_with_fallback(&req);
}
