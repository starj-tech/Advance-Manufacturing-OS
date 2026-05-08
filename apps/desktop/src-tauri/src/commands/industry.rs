use super::{CommandError, CommandResult};
use aether_industry::{profile_for, CapabilityKind, Industry, INDUSTRIES};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct IndustryDto {
    pub slug: String,
    pub display: String,
}

#[derive(Serialize)]
pub struct CapabilityDto {
    pub id: String,
    pub kind: String,
    pub display: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct ProfileDto {
    pub industry: IndustryDto,
    pub capabilities: Vec<CapabilityDto>,
    pub auto_modules: Vec<String>,
    pub default_standards: Vec<String>,
}

#[tauri::command]
pub async fn industry_list() -> CommandResult<Vec<IndustryDto>> {
    Ok(INDUSTRIES
        .iter()
        .map(|i| IndustryDto {
            slug: i.slug().to_string(),
            display: i.display().to_string(),
        })
        .collect())
}

#[derive(Deserialize)]
pub struct ProfileArgs {
    pub slug: String,
}

#[tauri::command]
pub async fn industry_profile(args: ProfileArgs) -> CommandResult<ProfileDto> {
    let industry = INDUSTRIES
        .iter()
        .copied()
        .find(|i| i.slug() == args.slug)
        .ok_or_else(|| CommandError::InvalidArgument(format!("unknown industry: {}", args.slug)))?;

    let p = profile_for(industry);
    Ok(ProfileDto {
        industry: IndustryDto {
            slug: industry.slug().to_string(),
            display: industry.display().to_string(),
        },
        capabilities: p
            .capabilities
            .into_iter()
            .map(|c| CapabilityDto {
                id: c.id.into(),
                kind: kind_slug(c.kind).into(),
                display: c.display.into(),
                description: c.description.into(),
            })
            .collect(),
        auto_modules: p.auto_modules.into_iter().map(String::from).collect(),
        default_standards: p.default_standards.into_iter().map(String::from).collect(),
    })
}

fn kind_slug(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Tracking => "tracking",
        CapabilityKind::Quality => "quality",
        CapabilityKind::Maintenance => "maintenance",
        CapabilityKind::Safety => "safety",
        CapabilityKind::Compliance => "compliance",
        CapabilityKind::Process => "process",
        CapabilityKind::Sustainability => "sustainability",
    }
}

#[derive(Deserialize)]
pub struct SetIndustryArgs {
    pub slug: String,
}

#[tauri::command]
pub async fn industry_set(_args: SetIndustryArgs) -> CommandResult<()> {
    // Real impl persists to `tenant_industry` row + triggers module
    // auto-install via aether-modules. Wired in PR #6.
    Err(CommandError::NotImplemented(
        "industry_set wired in PR #6 (tenant onboarding)".into(),
    ))
}

/// Demonstrates that the industry crate is reachable from the host
/// without exposing the full enum mapping yet.
#[allow(dead_code)]
fn _industry_used(_: Industry) {}
