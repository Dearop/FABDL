/// Shared types used across all contract modules.

pub type AccountId = [u8; 20];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Xrp,
    Issued,
}

#[derive(Clone, Copy)]
pub struct Asset {
    pub kind: AssetKind,
    pub code: [u8; 20],
    pub issuer: AccountId,
}

#[derive(Clone, Copy)]
pub struct PoolKey {
    pub asset0: Asset,
    pub asset1: Asset,
    pub fee_bps: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContractError {
    InvalidTickRange,
    TickSpacingViolation,
    SlippageLimitExceeded,
    NotAuthorized,
    Paused,
    MathOverflow,
    InvalidLiquidityDelta,
    PoolNotInitialized,
}

/// Error code mapping for ABI return values.
impl ContractError {
    pub fn code(&self) -> u32 {
        match self {
            ContractError::InvalidTickRange => 1,
            ContractError::TickSpacingViolation => 2,
            ContractError::SlippageLimitExceeded => 3,
            ContractError::NotAuthorized => 4,
            ContractError::Paused => 5,
            ContractError::MathOverflow => 6,
            ContractError::InvalidLiquidityDelta => 7,
            ContractError::PoolNotInitialized => 8,
        }
    }
}
