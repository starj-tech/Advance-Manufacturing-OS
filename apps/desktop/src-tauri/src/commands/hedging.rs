use super::{CommandError, CommandResult};
use serde::Serialize;

#[derive(Serialize)]
pub struct HedgeRecommendationDto {
    pub commodity: String,
    pub spot: f64,
    pub sma: f64,
    pub action: String,
    pub estimated_savings_pct: f64,
    pub rationale: String,
}

#[derive(Serialize)]
pub struct CommodityPriceDto {
    pub commodity: String,
    pub at: String,
    pub usd_price: f64,
    pub source: String,
}

#[tauri::command]
pub async fn hedging_recommendations() -> CommandResult<Vec<HedgeRecommendationDto>> {
    Err(CommandError::NotImplemented(
        "live commodity feed wired in PR #7 (analytics module)".into(),
    ))
}

#[tauri::command]
pub async fn hedging_commodity_prices(
    _commodity: String,
    _days: u32,
) -> CommandResult<Vec<CommodityPriceDto>> {
    Ok(vec![])
}
