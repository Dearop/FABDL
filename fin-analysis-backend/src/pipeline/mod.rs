/// Analysis pipeline — orchestrates XRPL queries and quant models.
pub mod analyze_risk;
pub mod check_position;
pub mod router;

use async_trait::async_trait;

use crate::{
    error::AnalysisError,
    quant::{DefaultQuantModel, QuantModel},
    types::{intent::IntentRouterOutput, quant::PortfolioRiskSummary},
    xrpl::XrplClient,
};

// ---------------------------------------------------------------------------
// AnalysisPipeline trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AnalysisPipeline: Send + Sync {
    async fn run(&self, intent: IntentRouterOutput) -> Result<PortfolioRiskSummary, AnalysisError>;
}

// ---------------------------------------------------------------------------
// DefaultPipeline
// ---------------------------------------------------------------------------

pub struct DefaultPipeline {
    pub xrpl: Box<dyn XrplClient>,
    pub quant: Box<dyn QuantModel>,
}

impl DefaultPipeline {
    pub fn new(xrpl: impl XrplClient + 'static) -> Self {
        Self {
            xrpl: Box::new(xrpl),
            quant: Box::new(DefaultQuantModel::default()),
        }
    }
}

#[async_trait]
impl AnalysisPipeline for DefaultPipeline {
    async fn run(&self, intent: IntentRouterOutput) -> Result<PortfolioRiskSummary, AnalysisError> {
        router::dispatch(&intent, self.xrpl.as_ref(), self.quant.as_ref()).await
    }
}

#[cfg(test)]
mod tests;
