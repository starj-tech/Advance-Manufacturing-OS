use super::{CommandError, CommandResult};
use aether_compliance::{StandardKind, CATALOG};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct StandardDto {
    pub slug: String,
    pub display: String,
    pub jurisdiction: String,
    pub kind: String,
    pub summary: String,
    pub control_point_count: usize,
}

#[tauri::command]
pub async fn compliance_standards() -> CommandResult<Vec<StandardDto>> {
    Ok(CATALOG
        .iter()
        .map(|s| StandardDto {
            slug: s.slug.into(),
            display: s.display.into(),
            jurisdiction: s.jurisdiction.into(),
            kind: kind_slug(s.kind).into(),
            summary: s.summary.into(),
            control_point_count: s.control_point_ids.len(),
        })
        .collect())
}

fn kind_slug(kind: StandardKind) -> &'static str {
    match kind {
        StandardKind::International => "international",
        StandardKind::Industry => "industry",
        StandardKind::Regulation => "regulation",
        StandardKind::National => "national",
    }
}

#[derive(Deserialize)]
pub struct RunArgs {
    pub standard_slug: String,
}

#[derive(Serialize)]
pub struct ReportSummaryDto {
    pub standard_slug: String,
    pub status: String,
    pub controls_total: usize,
    pub controls_passed: usize,
    pub controls_failed: usize,
    pub controls_needs_review: usize,
    pub generated_at: String,
}

#[tauri::command]
pub async fn compliance_run(_args: RunArgs) -> CommandResult<ReportSummaryDto> {
    Err(CommandError::NotImplemented(
        "compliance probes wired in PR #6 (per-control evidence collection)".into(),
    ))
}

#[tauri::command]
pub async fn compliance_latest_report(
    _standard_slug: String,
) -> CommandResult<Option<ReportSummaryDto>> {
    Ok(None)
}
