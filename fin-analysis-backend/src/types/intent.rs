/// Types for the small (on-device) LLM's structured output.
///
/// The intent router (Llama 3.2 3B, quantized) parses natural language and
/// emits one of these structs as JSON. The backend deserialises it and
/// dispatches to the appropriate pipeline variant.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct IntentRouterOutput {
    pub action: IntentAction,
    pub scope: IntentScope,
    pub parameters: IntentParameters,
    /// Model confidence in the parsed intent (0.0–1.0). Optional; may be
    /// absent if the router does not emit it.
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntentAction {
    AnalyzeRisk,
    ExecuteStrategy,
    CheckPosition,
    GetPrice,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntentScope {
    Portfolio,
    SpecificAsset,
    SpecificPool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct IntentParameters {
    /// XRPL wallet address of the user (r-address).
    pub wallet_address: Option<String>,
    /// Pool label, e.g. "XRP/USD".
    pub pool: Option<String>,
    /// Optional focus area, e.g. "impermanent_loss".
    pub focus: Option<String>,
}
