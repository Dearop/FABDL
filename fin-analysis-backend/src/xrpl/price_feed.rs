/// XRP/USD price fetching.
///
/// Primary source: CoinGecko simple price API (no auth required for low rate).
/// Fallback: returns a sensible cached value if the request fails.
use reqwest::Client;
use serde::Deserialize;

use crate::{error::AnalysisError, types::pool::PricePoint};

const COINGECKO_PRICE_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=ripple&vs_currencies=usd";

const COINGECKO_HISTORY_URL_TEMPLATE: &str =
    "https://api.coingecko.com/api/v3/coins/ripple/market_chart?vs_currency=usd&interval=daily&days=";

/// Fallback XRP/USD price used when CoinGecko is unavailable or rate-limiting.
const XRP_USD_FALLBACK: f64 = 2.50;

pub(super) async fn fetch_xrp_usd_price(
    http: &Client,
    _override_url: Option<&str>,
) -> Result<f64, AnalysisError> {
    #[derive(Deserialize)]
    struct CgPrice {
        ripple: CgUsd,
    }
    #[derive(Deserialize)]
    struct CgUsd {
        usd: f64,
    }

    let result = async {
        let resp: CgPrice = http
            .get(COINGECKO_PRICE_URL)
            .send()
            .await?
            .json()
            .await?;
        Ok::<f64, AnalysisError>(resp.ripple.usd)
    }
    .await;

    match result {
        Ok(price) => {
            tracing::info!(price, "XRP/USD price fetched from CoinGecko");
            Ok(price)
        }
        Err(e) => {
            tracing::warn!(error = %e, fallback = XRP_USD_FALLBACK, "CoinGecko unavailable — using fallback price");
            Ok(XRP_USD_FALLBACK)
        }
    }
}

pub(super) async fn fetch_price_history(
    http: &Client,
    days: u32,
    _override_url: Option<&str>,
) -> Result<Vec<PricePoint>, AnalysisError> {
    #[derive(Deserialize)]
    struct CgHistory {
        /// Each element: [timestamp_ms, price]
        prices: Vec<[f64; 2]>,
    }

    let url = format!("{COINGECKO_HISTORY_URL_TEMPLATE}{days}");
    let resp: CgHistory = match http.get(&url).send().await?.json().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "CoinGecko price history unavailable — returning empty history");
            return Ok(vec![]);
        }
    };

    let points = resp
        .prices
        .into_iter()
        .map(|p| PricePoint {
            timestamp_secs: (p[0] / 1000.0) as u64,
            xrp_usd: p[1],
        })
        .collect();

    Ok(points)
}
