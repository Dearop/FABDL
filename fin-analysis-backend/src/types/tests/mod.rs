use crate::types::{
    intent::{IntentAction, IntentRouterOutput, IntentScope},
    quant::{PortfolioRiskSummary, PositionRisk},
    xrpl::AmountField,
};

// ---------------------------------------------------------------------------
// IntentRouterOutput serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn round_trip_intent_router_output() {
    let json = r#"{
        "action": "analyze_risk",
        "scope": "portfolio",
        "parameters": {
            "wallet_address": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",
            "pool": "XRP/USD"
        },
        "confidence": 0.95
    }"#;

    let parsed: IntentRouterOutput = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.action, IntentAction::AnalyzeRisk);
    assert_eq!(parsed.scope, IntentScope::Portfolio);
    assert_eq!(
        parsed.parameters.wallet_address.as_deref(),
        Some("rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN")
    );
    assert_eq!(parsed.parameters.pool.as_deref(), Some("XRP/USD"));
    assert!((parsed.confidence.unwrap() - 0.95).abs() < 1e-5);

    // Re-serialise and re-parse: fields must survive the round-trip.
    let re_json = serde_json::to_string(&parsed).unwrap();
    let re_parsed: IntentRouterOutput = serde_json::from_str(&re_json).unwrap();
    assert_eq!(parsed, re_parsed);
}

#[test]
fn intent_router_output_no_confidence() {
    let json = r#"{"action":"get_price","scope":"specific_asset","parameters":{}}"#;
    let parsed: IntentRouterOutput = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.action, IntentAction::GetPrice);
    assert!(parsed.confidence.is_none());
}

#[test]
fn unknown_action_returns_error() {
    let json = r#"{"action":"launch_rockets","scope":"portfolio","parameters":{}}"#;
    let result: Result<IntentRouterOutput, _> = serde_json::from_str(json);
    assert!(result.is_err(), "unknown action should fail to deserialise");
}

// ---------------------------------------------------------------------------
// PortfolioRiskSummary serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn round_trip_portfolio_risk_summary() {
    let summary = PortfolioRiskSummary {
        total_value_usd: 50_000.0,
        impermanent_loss_pct: -2.3,
        impermanent_loss_usd: -1_150.0,
        delta_exposure_xrp: 1_500.0,
        delta_exposure_usd_if_down_10: -75.0,
        fee_income_7d: 120.0,
        current_xrp_price: 0.50,
        sharpe_ratio: 1.2,
        var_95_usd: 800.0,
        break_even_lower: 0.38,
        break_even_upper: 0.65,
        fee_apr: 0.15,
        positions: vec![PositionRisk {
            pool_label: "XRP/USD".to_string(),
            position_value_usd: 50_000.0,
            il_pct: -2.3,
            il_usd: -1_150.0,
            fee_apr: 0.15,
            fees_earned_7d_usd: 120.0,
            break_even_lower: 0.38,
            break_even_upper: 0.65,
            delta_xrp: 1_500.0,
            sharpe: 1.2,
            var_95_usd: 800.0,
            lp_share_pct: 0.025,
        }],
        lending_vaults: Vec::new(),
        open_loans: Vec::new(),
    };

    let json = serde_json::to_string(&summary).unwrap();
    let parsed: PortfolioRiskSummary = serde_json::from_str(&json).unwrap();
    assert!((parsed.total_value_usd - 50_000.0).abs() < 1e-6);
    assert_eq!(parsed.positions.len(), 1);
    assert_eq!(parsed.positions[0].pool_label, "XRP/USD");
}

// ---------------------------------------------------------------------------
// AmountField parsing
// ---------------------------------------------------------------------------

#[test]
fn amount_field_xrp_parses_drops() {
    let json = r#""1000000""#;
    let field: AmountField = serde_json::from_str(json).unwrap();
    match &field {
        AmountField::Xrp(s) => assert_eq!(s, "1000000"),
        _ => panic!("expected Xrp variant"),
    }
    assert_eq!(field.parse_raw(), 1_000_000u128);
    assert_eq!(field.currency(), "XRP");
    assert!(field.issuer().is_none());
}

#[test]
fn amount_field_token_parses_object() {
    let json = r#"{"value":"500.123456","currency":"USD","issuer":"rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh"}"#;
    let field: AmountField = serde_json::from_str(json).unwrap();
    match &field {
        AmountField::Token { currency, issuer, .. } => {
            assert_eq!(currency, "USD");
            assert_eq!(issuer, "rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh");
        }
        _ => panic!("expected Token variant"),
    }
    // 500.123456 * 1_000_000 = 500_123_456
    assert_eq!(field.parse_raw(), 500_123_456u128);
    assert_eq!(field.currency(), "USD");
    assert!(field.issuer().is_some());
}
