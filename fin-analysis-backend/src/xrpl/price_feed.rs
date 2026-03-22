/// XRP/USD price fetching.
///
/// Primary source: CoinGecko simple price API.
/// Fallback: returns a sensible cached value if the request fails.
use reqwest::Client;
use serde_json::Value;

use crate::{error::AnalysisError, types::pool::PricePoint};

const COINGECKO_PRICE_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=ripple&vs_currencies=usd";

const COINGECKO_HISTORY_URL_TEMPLATE: &str =
    "https://api.coingecko.com/api/v3/coins/ripple/market_chart?vs_currency=usd&interval=daily&days=";

/// Fallback XRP/USD price used when CoinGecko is unavailable or rate-limiting.
const XRP_USD_FALLBACK: f64 = 2.50;

fn truncate_body(body: &str) -> String {
    let snippet: String = body.chars().take(240).collect();
    if body.chars().count() > 240 {
        format!("{snippet}...")
    } else {
        snippet
    }
}

fn parse_price_body(body: &str) -> Result<f64, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    value
        .get("ripple")
        .and_then(|v| v.get("usd"))
        .and_then(|v| v.as_f64())
        .ok_or_else(|| format!("unexpected payload: {}", truncate_body(body)))
}

fn parse_history_body(body: &str) -> Result<Vec<PricePoint>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    let prices = value
        .get("prices")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("unexpected payload: {}", truncate_body(body)))?;

    let mut points = Vec::with_capacity(prices.len());
    for entry in prices {
        let pair = entry
            .as_array()
            .ok_or_else(|| format!("invalid price entry: {}", truncate_body(body)))?;
        if pair.len() != 2 {
            return Err(format!("invalid price tuple: {}", truncate_body(body)));
        }

        let timestamp_ms = pair[0]
            .as_f64()
            .ok_or_else(|| format!("invalid history timestamp: {}", truncate_body(body)))?;
        let price = pair[1]
            .as_f64()
            .ok_or_else(|| format!("invalid history price: {}", truncate_body(body)))?;

        points.push(PricePoint {
            timestamp_secs: (timestamp_ms / 1000.0) as u64,
            xrp_usd: price,
        });
    }

    Ok(points)
}

pub(super) async fn fetch_xrp_usd_price(
    http: &Client,
    override_url: Option<&str>,
) -> Result<f64, AnalysisError> {
    let url = override_url.unwrap_or(COINGECKO_PRICE_URL);
    let response = http.get(url).send().await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        tracing::warn!(
            status = %status,
            body = %truncate_body(&body),
            fallback = XRP_USD_FALLBACK,
            "CoinGecko price request returned non-success status"
        );
        return Ok(XRP_USD_FALLBACK);
    }

    match parse_price_body(&body) {
        Ok(price) => {
            tracing::info!(price, "XRP/USD price fetched from CoinGecko");
            Ok(price)
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                body = %truncate_body(&body),
                fallback = XRP_USD_FALLBACK,
                "CoinGecko price payload unavailable - using fallback price"
            );
            Ok(XRP_USD_FALLBACK)
        }
    }
}

pub(super) async fn fetch_price_history(
    http: &Client,
    days: u32,
    override_url: Option<&str>,
) -> Result<Vec<PricePoint>, AnalysisError> {
    let url = override_url
        .map(str::to_string)
        .unwrap_or_else(|| format!("{COINGECKO_HISTORY_URL_TEMPLATE}{days}"));

    let response = http.get(&url).send().await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        tracing::warn!(
            status = %status,
            body = %truncate_body(&body),
            "CoinGecko price history request returned non-success status - returning empty history"
        );
        return Ok(vec![]);
    }

    match parse_history_body(&body) {
        Ok(points) => Ok(points),
        Err(error) => {
            tracing::warn!(
                error = %error,
                body = %truncate_body(&body),
                "CoinGecko price history payload unavailable - returning empty history"
            );
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_history_body, parse_price_body};

    #[test]
    fn parse_price_body_happy_path() {
        let price = parse_price_body(r#"{"ripple":{"usd":2.42}}"#).unwrap();
        assert!((price - 2.42).abs() < 1e-9);
    }

    #[test]
    fn parse_price_body_rejects_error_payload() {
        let err = parse_price_body(r#"{"status":{"error_code":429}}"#).unwrap_err();
        assert!(err.contains("unexpected payload"));
    }

    #[test]
    fn parse_history_body_happy_path() {
        let history = parse_history_body(r#"{"prices":[[1000,2.0],[2000,2.2]]}"#).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].timestamp_secs, 1);
        assert!((history[1].xrp_usd - 2.2).abs() < 1e-9);
    }

    #[test]
    fn parse_history_body_rejects_missing_prices() {
        let err = parse_history_body(r#"{"status":"throttled"}"#).unwrap_err();
        assert!(err.contains("unexpected payload"));
    }
}
