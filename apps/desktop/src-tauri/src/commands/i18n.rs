use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct LocaleDto {
    pub tag: String,
    pub native_name: String,
    pub default_currency: String,
    pub default_tax_label: String,
}

#[tauri::command]
pub async fn i18n_locales() -> CommandResult<Vec<LocaleDto>> {
    Ok(vec![
        LocaleDto {
            tag: "en-US".into(),
            native_name: "English (US)".into(),
            default_currency: "USD".into(),
            default_tax_label: "Sales tax".into(),
        },
        LocaleDto {
            tag: "en-GB".into(),
            native_name: "English (UK)".into(),
            default_currency: "GBP".into(),
            default_tax_label: "VAT 20%".into(),
        },
        LocaleDto {
            tag: "id-ID".into(),
            native_name: "Bahasa Indonesia".into(),
            default_currency: "IDR".into(),
            default_tax_label: "PPN 11%".into(),
        },
        LocaleDto {
            tag: "de-DE".into(),
            native_name: "Deutsch".into(),
            default_currency: "EUR".into(),
            default_tax_label: "MwSt. 19%".into(),
        },
        LocaleDto {
            tag: "ja-JP".into(),
            native_name: "日本語".into(),
            default_currency: "JPY".into(),
            default_tax_label: "消費税 10%".into(),
        },
        LocaleDto {
            tag: "zh-CN".into(),
            native_name: "中文".into(),
            default_currency: "CNY".into(),
            default_tax_label: "增值税 13%".into(),
        },
    ])
}

#[derive(Deserialize)]
pub struct SetLocaleArgs {
    pub tag: String,
}

#[tauri::command]
pub async fn i18n_set_locale(_args: SetLocaleArgs) -> CommandResult<()> {
    // The locale persists per device in tauri-plugin-store (PR #6).
    Ok(())
}

#[derive(Deserialize)]
pub struct FormatMoneyArgs {
    pub minor: i64,
    pub currency: String,
    pub locale_tag: String,
}

#[tauri::command]
pub async fn i18n_format_money(args: FormatMoneyArgs) -> CommandResult<String> {
    // Frontend-only formatting via Intl.NumberFormat is preferred; this
    // command exists for headless / report-export use cases.
    let _ = (&args.locale_tag, &args.currency, args.minor);
    Err(CommandError::NotImplemented(
        "headless i18n_format_money wired in PR #6".into(),
    ))
}
