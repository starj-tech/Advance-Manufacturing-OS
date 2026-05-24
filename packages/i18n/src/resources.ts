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

  // Shell layout titles / subtitles
  'shell.developer.title': 'Developer',
  'shell.developer.subtitle': 'Infrastructure · Modules · Audit · Telemetry',
  'shell.executive.title': 'Executive',
  'shell.executive.subtitle': 'Vision · Strategy · Capital allocation',
  'shell.manager.title': 'Manager',
  'shell.manager.subtitle': 'Operations · Maintenance · Inventory · People',

  // Developer nav
  'nav.developer.infrastructure': 'Infrastructure',
  'nav.developer.discovery': 'IoT discovery',
  'nav.developer.industry': 'Industry profile',
  'nav.developer.modules': 'Module registry',
  'nav.developer.audit': 'Audit log',
  'nav.developer.health': 'System health',

  // Executive nav
  'nav.executive.overview': 'Overview',
  'nav.executive.digitalTwin': 'Digital twin',
  'nav.executive.projections': 'AI projections',
  'nav.executive.compliance': 'Compliance',

  // Manager nav
  'nav.manager.workOrders': 'Work orders',
  'nav.manager.machines': 'Machines',
  'nav.manager.maintenance': 'Maintenance',
  'nav.manager.inventory': 'Inventory',
  'nav.manager.supplyChain': 'Supply chain',
  'nav.manager.roster': 'Roster',
  'nav.manager.certifications': 'Certifications',
  'nav.manager.support': 'AI support',

  // Shared skeleton-page note (Placeholder default body)
  'common.skeletonNote': 'Skeleton page — the real implementation lands in a follow-up PR.',

  // Skeleton page titles / descriptions
  'page.audit.title': 'Audit log',
  'page.audit.desc':
    'Append-only ledger of every privileged action. Streamed via Supabase Realtime.',
  'page.modules.title': 'Module registry',
  'page.modules.desc': 'Browse, install, pin, and revoke modules across this tenant.',
  'page.projections.title': 'AI projections',
  'page.projections.desc': 'Profitability scenarios driven by historical telemetry + ML forecasts.',
  'page.maintenance.title': 'Predictive maintenance',
  'page.maintenance.desc': 'Vibration/thermal anomalies, RUL forecasts, work-order generation.',
  'page.roster.title': 'Roster',
  'page.roster.desc':
    'Operators on shift, certifications, presence (mDNS-driven), shift assignments.',
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

  'shell.developer.title': 'Ontwikkelaar',
  'shell.developer.subtitle': 'Infrastructuur · Modules · Audit · Telemetrie',
  'shell.executive.title': 'Directie',
  'shell.executive.subtitle': 'Visie · Strategie · Kapitaalallocatie',
  'shell.manager.title': 'Manager',
  'shell.manager.subtitle': 'Operatie · Onderhoud · Voorraad · Mensen',

  'nav.developer.infrastructure': 'Infrastructuur',
  'nav.developer.discovery': 'IoT-detectie',
  'nav.developer.industry': 'Sectorprofiel',
  'nav.developer.modules': 'Moduleregister',
  'nav.developer.audit': 'Auditlogboek',
  'nav.developer.health': 'Systeemstatus',

  'nav.executive.overview': 'Overzicht',
  'nav.executive.digitalTwin': 'Digitale tweeling',
  'nav.executive.projections': 'AI-prognoses',
  'nav.executive.compliance': 'Naleving',

  'nav.manager.workOrders': 'Werkorders',
  'nav.manager.machines': 'Machines',
  'nav.manager.maintenance': 'Onderhoud',
  'nav.manager.inventory': 'Voorraad',
  'nav.manager.supplyChain': 'Toeleveringsketen',
  'nav.manager.roster': 'Rooster',
  'nav.manager.certifications': 'Certificeringen',
  'nav.manager.support': 'AI-ondersteuning',

  'common.skeletonNote': 'Skeletpagina — de echte implementatie volgt in een latere PR.',

  'page.audit.title': 'Auditlogboek',
  'page.audit.desc':
    'Append-only logboek van elke bevoorrechte actie. Gestreamd via Supabase Realtime.',
  'page.modules.title': 'Moduleregister',
  'page.modules.desc': 'Modules bekijken, installeren, vastzetten en intrekken voor deze tenant.',
  'page.projections.title': 'AI-prognoses',
  'page.projections.desc':
    'Winstgevendheidsscenario’s op basis van historische telemetrie + ML-prognoses.',
  'page.maintenance.title': 'Voorspellend onderhoud',
  'page.maintenance.desc':
    'Trillings-/thermische afwijkingen, RUL-prognoses, generatie van werkorders.',
  'page.roster.title': 'Rooster',
  'page.roster.desc':
    'Operators in dienst, certificeringen, aanwezigheid (mDNS-gestuurd), dienstindelingen.',
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

  'shell.developer.title': 'Entwickler',
  'shell.developer.subtitle': 'Infrastruktur · Module · Audit · Telemetrie',
  'shell.executive.title': 'Geschäftsleitung',
  'shell.executive.subtitle': 'Vision · Strategie · Kapitalallokation',
  'shell.manager.title': 'Manager',
  'shell.manager.subtitle': 'Betrieb · Wartung · Bestand · Personal',

  'nav.developer.infrastructure': 'Infrastruktur',
  'nav.developer.discovery': 'IoT-Erkennung',
  'nav.developer.industry': 'Branchenprofil',
  'nav.developer.modules': 'Modulregister',
  'nav.developer.audit': 'Audit-Protokoll',
  'nav.developer.health': 'Systemzustand',

  'nav.executive.overview': 'Übersicht',
  'nav.executive.digitalTwin': 'Digitaler Zwilling',
  'nav.executive.projections': 'KI-Prognosen',
  'nav.executive.compliance': 'Compliance',

  'nav.manager.workOrders': 'Arbeitsaufträge',
  'nav.manager.machines': 'Maschinen',
  'nav.manager.maintenance': 'Wartung',
  'nav.manager.inventory': 'Bestand',
  'nav.manager.supplyChain': 'Lieferkette',
  'nav.manager.roster': 'Dienstplan',
  'nav.manager.certifications': 'Zertifizierungen',
  'nav.manager.support': 'KI-Support',

  'common.skeletonNote': 'Gerüstseite — die echte Implementierung folgt in einem späteren PR.',

  'page.audit.title': 'Audit-Protokoll',
  'page.audit.desc':
    'Append-only-Protokoll jeder privilegierten Aktion. Gestreamt über Supabase Realtime.',
  'page.modules.title': 'Modulregister',
  'page.modules.desc':
    'Module für diesen Mandanten durchsuchen, installieren, anheften und widerrufen.',
  'page.projections.title': 'KI-Prognosen',
  'page.projections.desc':
    'Rentabilitätsszenarien auf Basis historischer Telemetrie + ML-Prognosen.',
  'page.maintenance.title': 'Vorausschauende Wartung',
  'page.maintenance.desc':
    'Vibrations-/Thermalanomalien, RUL-Prognosen, Erzeugung von Arbeitsaufträgen.',
  'page.roster.title': 'Dienstplan',
  'page.roster.desc':
    'Operatoren im Dienst, Zertifizierungen, Anwesenheit (mDNS-gesteuert), Schichtzuweisungen.',
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

  'shell.developer.title': 'Pengembang',
  'shell.developer.subtitle': 'Infrastruktur · Modul · Audit · Telemetri',
  'shell.executive.title': 'Eksekutif',
  'shell.executive.subtitle': 'Visi · Strategi · Alokasi modal',
  'shell.manager.title': 'Manajer',
  'shell.manager.subtitle': 'Operasi · Pemeliharaan · Inventaris · Orang',

  'nav.developer.infrastructure': 'Infrastruktur',
  'nav.developer.discovery': 'Penemuan IoT',
  'nav.developer.industry': 'Profil industri',
  'nav.developer.modules': 'Registri modul',
  'nav.developer.audit': 'Log audit',
  'nav.developer.health': 'Kesehatan sistem',

  'nav.executive.overview': 'Ikhtisar',
  'nav.executive.digitalTwin': 'Kembar digital',
  'nav.executive.projections': 'Proyeksi AI',
  'nav.executive.compliance': 'Kepatuhan',

  'nav.manager.workOrders': 'Perintah kerja',
  'nav.manager.machines': 'Mesin',
  'nav.manager.maintenance': 'Pemeliharaan',
  'nav.manager.inventory': 'Inventaris',
  'nav.manager.supplyChain': 'Rantai pasok',
  'nav.manager.roster': 'Jadwal kerja',
  'nav.manager.certifications': 'Sertifikasi',
  'nav.manager.support': 'Dukungan AI',

  'common.skeletonNote': 'Halaman kerangka — implementasi sebenarnya menyusul di PR berikutnya.',

  'page.audit.title': 'Log audit',
  'page.audit.desc':
    'Catatan hanya-tambah untuk setiap tindakan berhak istimewa. Dialirkan via Supabase Realtime.',
  'page.modules.title': 'Registri modul',
  'page.modules.desc': 'Telusuri, pasang, sematkan, dan cabut modul di seluruh tenant ini.',
  'page.projections.title': 'Proyeksi AI',
  'page.projections.desc': 'Skenario profitabilitas berdasarkan telemetri historis + prakiraan ML.',
  'page.maintenance.title': 'Pemeliharaan prediktif',
  'page.maintenance.desc': 'Anomali getaran/termal, prakiraan RUL, pembuatan perintah kerja.',
  'page.roster.title': 'Jadwal kerja',
  'page.roster.desc':
    'Operator yang bertugas, sertifikasi, kehadiran (berbasis mDNS), penugasan shift.',
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

  'shell.developer.title': '開発者',
  'shell.developer.subtitle': 'インフラ · モジュール · 監査 · テレメトリ',
  'shell.executive.title': '経営層',
  'shell.executive.subtitle': 'ビジョン · 戦略 · 資本配分',
  'shell.manager.title': 'マネージャー',
  'shell.manager.subtitle': '運用 · 保全 · 在庫 · 人員',

  'nav.developer.infrastructure': 'インフラ',
  'nav.developer.discovery': 'IoT検出',
  'nav.developer.industry': '業種プロファイル',
  'nav.developer.modules': 'モジュールレジストリ',
  'nav.developer.audit': '監査ログ',
  'nav.developer.health': 'システム正常性',

  'nav.executive.overview': '概要',
  'nav.executive.digitalTwin': 'デジタルツイン',
  'nav.executive.projections': 'AI予測',
  'nav.executive.compliance': 'コンプライアンス',

  'nav.manager.workOrders': '作業指示',
  'nav.manager.machines': '設備',
  'nav.manager.maintenance': '保全',
  'nav.manager.inventory': '在庫',
  'nav.manager.supplyChain': 'サプライチェーン',
  'nav.manager.roster': '勤務表',
  'nav.manager.certifications': '認定',
  'nav.manager.support': 'AIサポート',

  'common.skeletonNote': 'スケルトンページ — 実装は今後のPRで対応します。',

  'page.audit.title': '監査ログ',
  'page.audit.desc': 'すべての特権操作の追記専用台帳。Supabase Realtimeでストリーミングされます。',
  'page.modules.title': 'モジュールレジストリ',
  'page.modules.desc': 'このテナント全体でモジュールを参照・インストール・固定・取り消しします。',
  'page.projections.title': 'AI予測',
  'page.projections.desc': '過去のテレメトリとML予測に基づく収益シナリオ。',
  'page.maintenance.title': '予知保全',
  'page.maintenance.desc': '振動・熱の異常、RUL予測、作業指示の生成。',
  'page.roster.title': '勤務表',
  'page.roster.desc': '勤務中のオペレーター、認定、在席（mDNSベース）、シフト割り当て。',
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

  'shell.developer.title': '开发者',
  'shell.developer.subtitle': '基础设施 · 模块 · 审计 · 遥测',
  'shell.executive.title': '高管',
  'shell.executive.subtitle': '愿景 · 战略 · 资本配置',
  'shell.manager.title': '经理',
  'shell.manager.subtitle': '运营 · 维护 · 库存 · 人员',

  'nav.developer.infrastructure': '基础设施',
  'nav.developer.discovery': 'IoT 发现',
  'nav.developer.industry': '行业配置',
  'nav.developer.modules': '模块注册表',
  'nav.developer.audit': '审计日志',
  'nav.developer.health': '系统健康',

  'nav.executive.overview': '概览',
  'nav.executive.digitalTwin': '数字孪生',
  'nav.executive.projections': 'AI 预测',
  'nav.executive.compliance': '合规',

  'nav.manager.workOrders': '工单',
  'nav.manager.machines': '设备',
  'nav.manager.maintenance': '维护',
  'nav.manager.inventory': '库存',
  'nav.manager.supplyChain': '供应链',
  'nav.manager.roster': '排班',
  'nav.manager.certifications': '认证',
  'nav.manager.support': 'AI 支持',

  'common.skeletonNote': '骨架页面 — 真实实现将在后续 PR 中完成。',

  'page.audit.title': '审计日志',
  'page.audit.desc': '每个特权操作的仅追加账本。通过 Supabase Realtime 流式传输。',
  'page.modules.title': '模块注册表',
  'page.modules.desc': '在此租户中浏览、安装、固定和吊销模块。',
  'page.projections.title': 'AI 预测',
  'page.projections.desc': '基于历史遥测 + ML 预测的盈利情景。',
  'page.maintenance.title': '预测性维护',
  'page.maintenance.desc': '振动/热异常、RUL 预测、工单生成。',
  'page.roster.title': '排班',
  'page.roster.desc': '在岗操作员、认证、在场（基于 mDNS）、班次分配。',
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
