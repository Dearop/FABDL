//! Execution adapter: routes swaps through Bedrock or direct XRPL path.
//!
//! DualPathAdapter tries the preferred path and falls back automatically.
//! Both paths share the same pre/post-validation logic so invariants hold
//! regardless of which execution backend is selected.

use uniswap_v3_xrpl_contract::types::ContractError;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPath {
    BedrockDirect,
    DirectXrpl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterError {
    /// Neither path is available.
    Unsupported,
    /// Path-level submission failed.
    SubmissionFailed,
    /// No response within deadline.
    Timeout,
    /// Contract-level error (e.g. slippage exceeded).
    ContractError(u32),
}

impl From<ContractError> for AdapterError {
    fn from(e: ContractError) -> Self {
        AdapterError::ContractError(e.code())
    }
}

#[derive(Debug, Clone)]
pub struct SwapRequest {
    /// Sender account id (20 bytes).
    pub sender: [u8; 20],
    /// Gross input amount.
    pub amount_in: u64,
    /// Minimum accepted output (slippage guard).
    pub min_amount_out: u64,
    /// true = token0 → token1 (price down), false = reverse.
    pub zero_for_one: bool,
    /// Hard price boundary in Q64.64.
    pub sqrt_price_limit: u128,
}

#[derive(Debug, Clone)]
pub struct SwapReceipt {
    pub path: ExecutionPath,
    pub amount_out: u64,
    /// On-chain transaction hash (32 bytes, zeroed when simulated).
    pub tx_hash: [u8; 32],
}

// ---------------------------------------------------------------------------
// Adapter configuration
// ---------------------------------------------------------------------------

pub struct DualPathAdapter {
    pub prefer_bedrock: bool,
    pub bedrock_available: bool,
    pub xrpl_available: bool,
}

impl DualPathAdapter {
    pub fn new(prefer_bedrock: bool, bedrock_available: bool, xrpl_available: bool) -> Self {
        Self { prefer_bedrock, bedrock_available, xrpl_available }
    }

    /// Determine the preferred execution path.
    pub fn choose_path(&self) -> Result<ExecutionPath, AdapterError> {
        if self.prefer_bedrock && self.bedrock_available {
            return Ok(ExecutionPath::BedrockDirect);
        }
        if self.xrpl_available {
            return Ok(ExecutionPath::DirectXrpl);
        }
        if self.bedrock_available {
            return Ok(ExecutionPath::BedrockDirect);
        }
        Err(AdapterError::Unsupported)
    }

    /// Execute with automatic fallback: try primary path, then secondary.
    pub fn execute_with_fallback(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError> {
        match self.choose_path()? {
            ExecutionPath::BedrockDirect => {
                match self.submit_bedrock(req) {
                    Ok(r) => Ok(r),
                    Err(_) if self.xrpl_available => self.submit_xrpl(req),
                    Err(e) => Err(e),
                }
            }
            ExecutionPath::DirectXrpl => self.submit_xrpl(req),
        }
    }

    // -----------------------------------------------------------------------
    // Bedrock path
    // -----------------------------------------------------------------------

    fn submit_bedrock(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError> {
        if !self.bedrock_available {
            return Err(AdapterError::Unsupported);
        }

        // Pre-validation: enforce slippage cap before sending to chain.
        self.validate_slippage(req)?;

        // Call the contract's swap_exact_in function via Bedrock transport.
        // In production: serialize `req` into a Bedrock `call` transaction,
        // broadcast it, and parse the receipt.
        //
        // For this MVP / test harness: delegate to the in-process contract.
        let amount_out = uniswap_v3_xrpl_contract::swap_exact_in(
            req.sender,
            req.amount_in,
            req.min_amount_out,
            if req.zero_for_one { 1 } else { 0 },
            req.sqrt_price_limit,
            0, // timestamp: 0 in adapter (not block-time aware)
        );

        if amount_out == 0 && req.min_amount_out > 0 {
            return Err(AdapterError::ContractError(
                ContractError::SlippageLimitExceeded.code(),
            ));
        }

        Ok(SwapReceipt {
            path: ExecutionPath::BedrockDirect,
            amount_out,
            tx_hash: [0u8; 32],
        })
    }

    // -----------------------------------------------------------------------
    // Direct XRPL path
    // -----------------------------------------------------------------------

    fn submit_xrpl(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError> {
        if !self.xrpl_available {
            return Err(AdapterError::Unsupported);
        }

        self.validate_slippage(req)?;

        // In production: construct AMMSwap / multi-step XRPL transactions,
        // sign with caller's key (never held by this adapter), broadcast,
        // and parse the ledger receipt.
        //
        // For this MVP: delegate to the in-process contract, same as Bedrock.
        let amount_out = uniswap_v3_xrpl_contract::swap_exact_in(
            req.sender,
            req.amount_in,
            req.min_amount_out,
            if req.zero_for_one { 1 } else { 0 },
            req.sqrt_price_limit,
            0, // timestamp: 0 in adapter (not block-time aware)
        );

        if amount_out == 0 && req.min_amount_out > 0 {
            return Err(AdapterError::ContractError(
                ContractError::SlippageLimitExceeded.code(),
            ));
        }

        Ok(SwapReceipt {
            path: ExecutionPath::DirectXrpl,
            amount_out,
            tx_hash: [0u8; 32],
        })
    }

    // -----------------------------------------------------------------------
    // Shared validation
    // -----------------------------------------------------------------------

    /// Enforce the 1% hard slippage cap before any submission.
    fn validate_slippage(&self, req: &SwapRequest) -> Result<(), AdapterError> {
        if req.amount_in == 0 {
            return Err(AdapterError::ContractError(
                ContractError::InvalidLiquidityDelta.code(),
            ));
        }
        // min_amount_out must be at least 99% of amount_in (1% max slippage).
        let floor = req.amount_in as u128 * 99 / 100;
        if (req.min_amount_out as u128) < floor {
            return Err(AdapterError::ContractError(
                ContractError::SlippageLimitExceeded.code(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uniswap_v3_xrpl_contract::{test_setup, math::Q64};

    fn owner() -> [u8; 20] { [7u8; 20] }

    fn setup_pool() {
        test_setup(owner(), 10);
        // Initialize pool at price = 1.0 (sqrt_price = Q64).
        uniswap_v3_xrpl_contract::initialize_pool(owner(), Q64, 30, 0, 0, 0);
        // Add liquidity so swaps produce output.
        uniswap_v3_xrpl_contract::mint(owner(), -1000, 1000, 1_000_000_000);
    }

    fn req(amount_in: u64, zero_for_one: bool) -> SwapRequest {
        SwapRequest {
            sender: owner(),
            amount_in,
            min_amount_out: amount_in * 99 / 100,
            zero_for_one,
            sqrt_price_limit: if zero_for_one { Q64 / 2 } else { Q64 * 2 },
        }
    }

    #[test]
    fn bedrock_path_executes() {
        setup_pool();
        let adapter = DualPathAdapter::new(true, true, false);
        let receipt = adapter.execute_with_fallback(&req(10_000, false)).unwrap();
        assert_eq!(receipt.path, ExecutionPath::BedrockDirect);
        assert!(receipt.amount_out > 0);
    }

    #[test]
    fn falls_back_to_direct_xrpl() {
        setup_pool();
        let adapter = DualPathAdapter::new(true, false, true);
        let receipt = adapter.execute_with_fallback(&req(10_000, false)).unwrap();
        assert_eq!(receipt.path, ExecutionPath::DirectXrpl);
    }

    #[test]
    fn no_paths_available_errors() {
        let adapter = DualPathAdapter::new(true, false, false);
        let err = adapter.execute_with_fallback(&req(1_000, false)).unwrap_err();
        assert_eq!(err, AdapterError::Unsupported);
    }

    #[test]
    fn slippage_cap_rejected_before_submission() {
        setup_pool();
        let adapter = DualPathAdapter::new(true, true, false);
        let bad_req = SwapRequest {
            sender: owner(),
            amount_in: 10_000,
            min_amount_out: 1, // essentially no slippage protection → should fail our guard
            zero_for_one: false,
            sqrt_price_limit: Q64 * 2,
        };
        let err = adapter.execute_with_fallback(&bad_req).unwrap_err();
        assert!(matches!(err, AdapterError::ContractError(_)));
    }

    #[test]
    fn zero_input_rejected() {
        let adapter = DualPathAdapter::new(true, true, false);
        let err = adapter.execute_with_fallback(&SwapRequest {
            sender: owner(),
            amount_in: 0,
            min_amount_out: 0,
            zero_for_one: false,
            sqrt_price_limit: Q64 * 2,
        }).unwrap_err();
        assert!(matches!(err, AdapterError::ContractError(_)));
    }
}
