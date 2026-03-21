/// Raw XRPL JSON-RPC response shapes.
///
/// These types map 1-to-1 to the XRPL wire format and are used only for
/// deserialisation. Higher-level code works with normalised `PoolSnapshot` /
/// `PositionSnapshot` from `types::pool`.
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// amm_info response
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct AmmInfoResponse {
    pub amm: AmmInfo,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AmmInfo {
    pub account: String,
    /// XRP side: string of drops, or token side: token object.
    pub amount: AmountField,
    /// The other asset.
    pub amount2: AmountField,
    pub lp_token: LpTokenInfo,
    /// Trading fee in basis points (0–1000).
    pub trading_fee: u16,
    pub auction_slot: Option<AuctionSlot>,
    pub vote_slots: Option<Vec<Value>>,
}

/// XRPL amounts are polymorphic: XRP is a decimal string (drops), issued
/// tokens are an object `{ value, currency, issuer }`.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum AmountField {
    /// XRP in drops, represented as a decimal string.
    Xrp(String),
    /// Issued token.
    Token {
        value: String,
        currency: String,
        issuer: String,
    },
}

impl AmountField {
    /// Parse the value as a `u128` of smallest units (drops for XRP,
    /// raw integer-scaled for tokens).
    pub fn parse_raw(&self) -> u128 {
        match self {
            AmountField::Xrp(s) => s.parse::<u128>().unwrap_or(0),
            AmountField::Token { value, .. } => {
                // XRPL token amounts are floating-point strings; multiply by
                // a canonical scale (1e6) to get integer representation.
                // For analysis purposes 6 decimal precision is sufficient.
                let v: f64 = value.parse().unwrap_or(0.0);
                (v * 1_000_000.0) as u128
            }
        }
    }

    pub fn currency(&self) -> &str {
        match self {
            AmountField::Xrp(_) => "XRP",
            AmountField::Token { currency, .. } => currency,
        }
    }

    pub fn issuer(&self) -> Option<&str> {
        match self {
            AmountField::Xrp(_) => None,
            AmountField::Token { issuer, .. } => Some(issuer),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LpTokenInfo {
    pub value: String,
    pub currency: String,
    pub issuer: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuctionSlot {
    pub price: AmountField,
    pub time_interval: Option<u32>,
    pub expiration: Option<u32>,
}

// ---------------------------------------------------------------------------
// account_lines response
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AccountLinesResponse {
    pub lines: Vec<TrustLine>,
    pub marker: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TrustLine {
    pub currency: String,
    /// XRPL returns this field as "account" (the counterparty address).
    #[serde(rename = "account")]
    pub issuer: String,
    /// Positive = asset held, negative = liability.
    pub balance: String,
    pub limit: String,
    pub limit_peer: Option<String>,
}

impl TrustLine {
    pub fn balance_f64(&self) -> f64 {
        self.balance.parse().unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// account_tx response
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AccountTxResponse {
    pub transactions: Vec<TxEntry>,
    pub marker: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TxEntry {
    /// Raw transaction object — relevant fields extracted in `xrpl::amm`.
    pub tx: Value,
    /// Transaction metadata.
    pub meta: Option<Value>,
}
