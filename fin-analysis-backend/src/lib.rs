//! `fin-analysis-backend` — quantitative LP analysis for the XRPL AMM advisor.
//!
//! # Architecture
//!
//! ```text
//! IntentRouterOutput (small LLM JSON)
//!        │
//!        ▼
//!   pipeline::router   ← dispatch by IntentAction
//!        │
//!        ├─ analyze_risk    ─┐
//!        ├─ check_position  ─┤─ xrpl::XrplClient   ← XRPL JSON-RPC
//!        └─ get_price       ─┘       │
//!                                    ▼
//!                             quant::QuantModel   ← IL, fees, VaR, Sharpe, …
//!                                    │
//!                                    ▼
//!                        PortfolioRiskSummary (large LLM input JSON)
//! ```

pub mod error;
pub mod pipeline;
pub mod quant;
pub mod server;
pub mod types;
pub mod xrpl;

pub use error::AnalysisError;
pub use pipeline::{AnalysisPipeline, DefaultPipeline};
pub use types::quant::PortfolioRiskSummary;
