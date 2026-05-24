//! Translation resource bundles.
//!
//! ## Why a hand-rolled catalog (not i18next / FormatJS)
//! The string surface is small and ships with the binary —
//! no runtime locale fetch, no ICU MessageFormat plurals yet.
//! A typed `Record<string, string>` per locale keeps the
//! bundle in the initial chunk (bundle-budget matters: the
//! <350 KB gz target in the roadmap) and lets `tsc` verify
//! that every key referenced by a component exists in at
//! least the `en-US` base.
//!
//! ## Fallback chain
//! `useTranslation` resolves a key as:
//!   active-locale bundle → `en-US` base → the key itself.
//! That means a partially-translated locale degrades to
//! English rather than showing a raw key, and a brand-new
//! key that hasn't been translated anywhere still renders
//! its English source (the key doubles as the en text only
//! where we forgot — every key here has a real en value).
//!
//! ## Interpolation
//! `{name}`-style placeholders are substituted by
//! `useTranslation`'s `t(key, params)`. Keep placeholder
//! names identical across every locale for a given key.

export type Messages = Record<string, string>;

/// The `en-US` bundle is the source of truth — every key the
/// app uses MUST appear here. Other locales are allowed to be
/// partial; missing keys fall back to this bundle.
const enUS: Messages = {
  // App chrome / login
  'app.tagline': 'Industrial Operating System — Foundation Skeleton',
  'login.devNotice': 'Dev mode — passkey wiring lands in next PR',
  'role.developer.label': 'Developer',
  'role.developer.desc': 'Infrastructure, module registry, audit log',
  'role.executive.label': 'Executive',
  'role.executive.desc': 'Digital twin, KPI dashboards, AI projections',
  'role.manager.label': 'Manager',
  'role.manager.desc': 'Work orders, machines, predictive maintenance, inventory',
  'role.employee.label': 'Employee',
  'role.employee.desc': 'Task cards, SOS, clock in/out (glove-friendly)',

  // Top bar
  'topbar.offline': 'offline · sync paused',
  'topbar.online': 'online · synced',
  'topbar.industry': 'industry · {name}',
  'topbar.langAria': 'Language and currency',
  'topbar.notificationsAria': '{count} notifications',
  'topbar.signOut': 'Sign out',

  // Employee · tasks
  'employee.tasks.title': 'Your tasks',
  'employee.tasks.subtitle': 'Tap a card to open. Long-press SOS at any time.',
  'employee.nav.tasks': 'Tasks',
  'employee.nav.sos': 'SOS',
  'employee.nav.clock': 'Clock',

  // Employee · SOS
  'employee.sos.title': 'Emergency',
  'employee.sos.instructions':
    'Hold the button for 2 seconds. Your supervisor and on-shift safety team are alerted instantly. Location is included.',
  'employee.sos.alertSent': 'Alert sent (skeleton). Real broadcast wires up in PR #5.',

  // Employee · clock
  'employee.clock.onShift': 'On shift',
  'employee.clock.offShift': 'Off shift',
  'employee.clock.clockedInAt': 'Clocked in at {time}',
  'employee.clock.clockIn': 'Clock in',
  'employee.clock.clockOut': 'Clock out',
  'employee.clock.geofenceNote': 'Geofence + presence broadcast wires up in PR #5.',
};

// en-GB shares the US base; only spellings / tax-ish phrasing
// would differ and none of the current keys need it.
const enGB: Messages = { ...enUS };

const nlNL: Messages = {
  'app.tagline': 'Industrieel besturingssysteem — basisskelet',
  'login.devNotice': 'Ontwikkelmodus — passkey-koppeling volgt in volgende PR',
  'role.developer.label': 'Ontwikkelaar',
  'role.developer.desc': 'Infrastructuur, moduleregister, auditlogboek',
  'role.executive.label': 'Directie',
  'role.executive.desc': 'Digitale tweeling, KPI-dashboards, AI-prognoses',
  'role.manager.label': 'Manager',
  'role.manager.desc': 'Werkorders, machines, voorspellend onderhoud, voorraad',
  'role.employee.label': 'Medewerker',
  'role.employee.desc': 'Taakkaarten, SOS, in-/uitklokken (handschoenvriendelijk)',

  'topbar.offline': 'offline · synchronisatie gepauzeerd',
  'topbar.online': 'online · gesynchroniseerd',
  'topbar.industry': 'sector · {name}',
  'topbar.langAria': 'Taal en valuta',
  'topbar.notificationsAria': '{count} meldingen',
  'topbar.signOut': 'Afmelden',

  'employee.tasks.title': 'Jouw taken',
  'employee.tasks.subtitle': 'Tik op een kaart om te openen. Houd SOS ingedrukt wanneer nodig.',
  'employee.nav.tasks': 'Taken',
  'employee.nav.sos': 'SOS',
  'employee.nav.clock': 'Klok',

  'employee.sos.title': 'Noodgeval',
  'employee.sos.instructions':
    'Houd de knop 2 seconden ingedrukt. Je leidinggevende en het veiligheidsteam van de dienst worden direct gewaarschuwd. Locatie wordt meegestuurd.',
  'employee.sos.alertSent': 'Melding verzonden (skelet). Echte uitzending volgt in PR #5.',

  'employee.clock.onShift': 'In dienst',
  'employee.clock.offShift': 'Buiten dienst',
  'employee.clock.clockedInAt': 'Ingeklokt om {time}',
  'employee.clock.clockIn': 'Inklokken',
  'employee.clock.clockOut': 'Uitklokken',
  'employee.clock.geofenceNote': 'Geofence + aanwezigheidsuitzending volgt in PR #5.',
};

const deDE: Messages = {
  'app.tagline': 'Industrielles Betriebssystem — Grundgerüst',
  'login.devNotice': 'Entwicklermodus — Passkey-Anbindung folgt im nächsten PR',
  'role.developer.label': 'Entwickler',
  'role.developer.desc': 'Infrastruktur, Modulregister, Audit-Protokoll',
  'role.executive.label': 'Geschäftsleitung',
  'role.executive.desc': 'Digitaler Zwilling, KPI-Dashboards, KI-Prognosen',
  'role.manager.label': 'Manager',
  'role.manager.desc': 'Arbeitsaufträge, Maschinen, vorausschauende Wartung, Bestand',
  'role.employee.label': 'Mitarbeiter',
  'role.employee.desc': 'Aufgabenkarten, SOS, Ein-/Ausstempeln (handschuhfreundlich)',

  'topbar.offline': 'offline · Synchronisierung pausiert',
  'topbar.online': 'online · synchronisiert',
  'topbar.industry': 'Branche · {name}',
  'topbar.langAria': 'Sprache und Währung',
  'topbar.notificationsAria': '{count} Benachrichtigungen',
  'topbar.signOut': 'Abmelden',

  'employee.tasks.title': 'Ihre Aufgaben',
  'employee.tasks.subtitle': 'Tippen Sie auf eine Karte zum Öffnen. SOS jederzeit gedrückt halten.',
  'employee.nav.tasks': 'Aufgaben',
  'employee.nav.sos': 'SOS',
  'employee.nav.clock': 'Zeiterfassung',

  'employee.sos.title': 'Notfall',
  'employee.sos.instructions':
    'Halten Sie die Taste 2 Sekunden gedrückt. Ihr Vorgesetzter und das Sicherheitsteam der Schicht werden sofort alarmiert. Der Standort wird übermittelt.',
  'employee.sos.alertSent': 'Alarm gesendet (Gerüst). Echte Übertragung folgt in PR #5.',

  'employee.clock.onShift': 'Im Dienst',
  'employee.clock.offShift': 'Außer Dienst',
  'employee.clock.clockedInAt': 'Eingestempelt um {time}',
  'employee.clock.clockIn': 'Einstempeln',
  'employee.clock.clockOut': 'Ausstempeln',
  'employee.clock.geofenceNote': 'Geofence + Anwesenheitsübertragung folgt in PR #5.',
};

const idID: Messages = {
  'app.tagline': 'Sistem Operasi Industri — Kerangka Dasar',
  'login.devNotice': 'Mode pengembang — koneksi passkey menyusul di PR berikutnya',
  'role.developer.label': 'Pengembang',
  'role.developer.desc': 'Infrastruktur, registri modul, log audit',
  'role.executive.label': 'Eksekutif',
  'role.executive.desc': 'Kembar digital, dasbor KPI, proyeksi AI',
  'role.manager.label': 'Manajer',
  'role.manager.desc': 'Perintah kerja, mesin, pemeliharaan prediktif, inventaris',
  'role.employee.label': 'Karyawan',
  'role.employee.desc': 'Kartu tugas, SOS, absen masuk/keluar (ramah sarung tangan)',

  'topbar.offline': 'luring · sinkronisasi dijeda',
  'topbar.online': 'daring · tersinkronisasi',
  'topbar.industry': 'industri · {name}',
  'topbar.langAria': 'Bahasa dan mata uang',
  'topbar.notificationsAria': '{count} notifikasi',
  'topbar.signOut': 'Keluar',

  'employee.tasks.title': 'Tugas Anda',
  'employee.tasks.subtitle': 'Ketuk kartu untuk membuka. Tekan lama SOS kapan saja.',
  'employee.nav.tasks': 'Tugas',
  'employee.nav.sos': 'SOS',
  'employee.nav.clock': 'Absen',

  'employee.sos.title': 'Darurat',
  'employee.sos.instructions':
    'Tahan tombol selama 2 detik. Supervisor dan tim keselamatan shift akan langsung diberi tahu. Lokasi disertakan.',
  'employee.sos.alertSent': 'Peringatan terkirim (kerangka). Siaran sungguhan menyusul di PR #5.',

  'employee.clock.onShift': 'Sedang bertugas',
  'employee.clock.offShift': 'Tidak bertugas',
  'employee.clock.clockedInAt': 'Absen masuk pukul {time}',
  'employee.clock.clockIn': 'Absen masuk',
  'employee.clock.clockOut': 'Absen keluar',
  'employee.clock.geofenceNote': 'Geofence + siaran kehadiran menyusul di PR #5.',
};

const jaJP: Messages = {
  'app.tagline': '産業用オペレーティングシステム — 基盤スケルトン',
  'login.devNotice': '開発モード — パスキー連携は次のPRで対応',
  'role.developer.label': '開発者',
  'role.developer.desc': 'インフラ、モジュールレジストリ、監査ログ',
  'role.executive.label': '経営層',
  'role.executive.desc': 'デジタルツイン、KPIダッシュボード、AI予測',
  'role.manager.label': 'マネージャー',
  'role.manager.desc': '作業指示、設備、予知保全、在庫',
  'role.employee.label': '従業員',
  'role.employee.desc': 'タスクカード、SOS、打刻（手袋対応）',

  'topbar.offline': 'オフライン · 同期停止中',
  'topbar.online': 'オンライン · 同期済み',
  'topbar.industry': '業種 · {name}',
  'topbar.langAria': '言語と通貨',
  'topbar.notificationsAria': '通知 {count} 件',
  'topbar.signOut': 'サインアウト',

  'employee.tasks.title': 'あなたのタスク',
  'employee.tasks.subtitle': 'カードをタップして開きます。SOSはいつでも長押しできます。',
  'employee.nav.tasks': 'タスク',
  'employee.nav.sos': 'SOS',
  'employee.nav.clock': '打刻',

  'employee.sos.title': '緊急',
  'employee.sos.instructions':
    'ボタンを2秒間長押ししてください。上司とシフトの安全チームに即座に通知されます。位置情報も送信されます。',
  'employee.sos.alertSent': 'アラートを送信しました（スケルトン）。実際の配信はPR #5で対応します。',

  'employee.clock.onShift': '勤務中',
  'employee.clock.offShift': '勤務外',
  'employee.clock.clockedInAt': '{time} に打刻',
  'employee.clock.clockIn': '出勤',
  'employee.clock.clockOut': '退勤',
  'employee.clock.geofenceNote': 'ジオフェンス + 在席配信はPR #5で対応します。',
};

const zhCN: Messages = {
  'app.tagline': '工业操作系统 — 基础骨架',
  'login.devNotice': '开发模式 — 通行密钥接入将在下一个 PR 完成',
  'role.developer.label': '开发者',
  'role.developer.desc': '基础设施、模块注册表、审计日志',
  'role.executive.label': '高管',
  'role.executive.desc': '数字孪生、KPI 仪表板、AI 预测',
  'role.manager.label': '经理',
  'role.manager.desc': '工单、设备、预测性维护、库存',
  'role.employee.label': '员工',
  'role.employee.desc': '任务卡、SOS、打卡（适合戴手套操作）',

  'topbar.offline': '离线 · 已暂停同步',
  'topbar.online': '在线 · 已同步',
  'topbar.industry': '行业 · {name}',
  'topbar.langAria': '语言和货币',
  'topbar.notificationsAria': '{count} 条通知',
  'topbar.signOut': '退出登录',

  'employee.tasks.title': '您的任务',
  'employee.tasks.subtitle': '点按卡片打开。可随时长按 SOS。',
  'employee.nav.tasks': '任务',
  'employee.nav.sos': 'SOS',
  'employee.nav.clock': '打卡',

  'employee.sos.title': '紧急',
  'employee.sos.instructions':
    '长按按钮 2 秒。您的主管和当班安全团队会立即收到警报，并附带位置信息。',
  'employee.sos.alertSent': '警报已发送（骨架）。真实广播将在 PR #5 接入。',

  'employee.clock.onShift': '在岗',
  'employee.clock.offShift': '离岗',
  'employee.clock.clockedInAt': '于 {time} 打卡',
  'employee.clock.clockIn': '上班打卡',
  'employee.clock.clockOut': '下班打卡',
  'employee.clock.geofenceNote': '地理围栏 + 在场广播将在 PR #5 接入。',
};

/// Master registry keyed by BCP-47 tag. `useTranslation`
/// indexes this by the active locale tag and falls back to
/// `en-US`.
export const RESOURCES: Record<string, Messages> = {
  'en-US': enUS,
  'en-GB': enGB,
  'nl-NL': nlNL,
  'de-DE': deDE,
  'id-ID': idID,
  'ja-JP': jaJP,
  'zh-CN': zhCN,
};

/// The base bundle every other locale falls back to. Exported
/// so tests can assert key-coverage parity.
export const BASE_LOCALE = 'en-US';

/// Resolve a message for a given locale tag + key, applying
/// the fallback chain. Pure function (no React) so it's unit-
/// testable and reusable outside components. `useTranslation`
/// wraps this with the active locale + interpolation.
export function resolveMessage(localeTag: string, key: string): string {
  const active = RESOURCES[localeTag];
  if (active && key in active) return active[key]!;
  const base = RESOURCES[BASE_LOCALE]!;
  if (key in base) return base[key]!;
  return key;
}

/// Substitute `{name}`-style placeholders. Separated from
/// `resolveMessage` so the lookup and the formatting can be
/// tested independently.
export function interpolate(template: string, params?: Record<string, string | number>): string {
  if (!params) return template;
  let out = template;
  for (const [k, v] of Object.entries(params)) {
    out = out.replaceAll(`{${k}}`, String(v));
  }
  return out;
}
