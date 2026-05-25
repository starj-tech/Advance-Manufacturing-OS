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

  // Executive · overview
  'page.overview.title': 'Overview',
  'page.overview.kpi.oee': 'OEE (overall)',
  'page.overview.kpi.throughput': 'Throughput (units/h)',
  'page.overview.kpi.quality': 'Quality first-pass',
  'page.overview.kpi.margin': 'Margin (rolling 30d)',
  'page.overview.hint.telemetry': 'wired in PR #3',
  'page.overview.hint.finance': 'finance feed in PR #5',

  // Executive · digital twin
  'page.digitalTwin.title': 'Digital twin',
  'page.digitalTwin.viewerTitle': 'Plant viewer',
  'page.digitalTwin.viewerSubtitle': 'React Three Fiber scene mounts here in PR #6',
  'page.digitalTwin.placeholder': 'Placeholder · 3D model + live telemetry overlay coming in PR #6',

  // Executive · compliance
  'page.compliance.title': 'Compliance attestation',
  'page.compliance.intro':
    "Live status across every regulatory + voluntary standard you've enrolled in. Each report is signed by the AETHER attestation service so external auditors can verify it without trusting us.",
  'page.compliance.rollup.enrolled': 'Enrolled',
  'page.compliance.rollup.compliant': 'Compliant',
  'page.compliance.rollup.needsReview': 'Needs review',
  'page.compliance.rollup.nonCompliant': 'Non-compliant',
  'page.compliance.standardsTitle': 'Standards',
  'page.compliance.standardsSubtitle':
    'Demo data — real probes wire up in PR #6 alongside the attestation signing key.',
  'page.compliance.col.standard': 'Standard',
  'page.compliance.col.jurisdiction': 'Jurisdiction',
  'page.compliance.col.kind': 'Kind',
  'page.compliance.col.controls': 'Controls',
  'page.compliance.col.status': 'Status',
  'page.compliance.col.lastReport': 'Last report',
  'page.compliance.status.compliant': 'Compliant',
  'page.compliance.status.needs-review': 'Needs review',
  'page.compliance.status.non-compliant': 'Non-compliant',
  'page.compliance.status.not-run': 'Not run',
  'page.compliance.kind.international': 'International',
  'page.compliance.kind.industry': 'Industry',
  'page.compliance.kind.regulation': 'Regulation',
  'page.compliance.kind.national': 'National',

  // Developer · infrastructure
  'page.infra.title': 'Infrastructure',
  'page.infra.note':
    'Service health metrics arrive in PR #2 once aether-telemetry is wired into the Tauri runtime.',

  // Developer · discovery
  'page.discovery.title': 'IoT Auto-Discovery',
  'page.discovery.intro':
    'Scan the local network for OPC-UA, MQTT, Modbus, and EtherNet/IP devices. Suggested tag bindings appear next to each match.',
  'page.discovery.scan': 'Scan network',
  'page.discovery.scanning': 'Scanning…',
  'page.discovery.scanningNote':
    'Probing /24 with 64-way concurrency · 4 protocol probes · 1.5s/host timeout',
  'page.discovery.emptyNote':
    'No scan in progress. The skeleton displays demo results when you click "Scan network". Real probing wires up in PR #3 alongside the protocol bridges.',
  'page.discovery.bindings': '{count} suggested tag bindings · fingerprint',
  'page.discovery.addToInventory': 'Add to inventory (PR #3)',

  // Developer · system health
  'page.health.title': 'System health',
  'page.health.intro':
    'Sync lag, outbox depth, telemetry throughput, plus the Neural Auto-Healing event ledger.',
  'page.health.kpi.outbox': 'Outbox depth',
  'page.health.kpi.syncLag': 'Sync lag (p99)',
  'page.health.kpi.telemetryRate': 'Telemetry rate',
  'page.health.kpi.diskFree': 'Disk free',
  'page.health.kpiHint': 'wired in PR #3',
  'page.health.ledgerTitle': 'Healing ledger',
  'page.health.ledgerSubtitle':
    'Append-only audit of every automatic remediation. Demo data shown.',
  'page.health.col.when': 'When',
  'page.health.col.source': 'Source',
  'page.health.col.symptom': 'Symptom',
  'page.health.col.policy': 'Policy',
  'page.health.col.outcome': 'Outcome',
  'page.health.outcome.applied': 'applied',
  'page.health.outcome.skipped': 'skipped',
  'page.health.outcome.failed': 'failed',

  // Developer · industry profile
  'page.industry.title': 'Industry profile',
  'page.industry.intro':
    "Pick the tenant's industry. AETHER-OS auto-activates capabilities, default modules, and compliance standards — no per-customer fork. 21 verticals supported. The picker below drives the live CapabilityProvider; switch to Manager → Inventory to see the columns reshape.",
  'page.industry.more': '+ 18 more (PR #6)',
  'page.industry.capsSuffix': '{count} auto-capabilities',
  'page.industry.capsSubtitle': 'Activated automatically when this profile is set on the tenant.',
  'page.industry.modulesTitle': 'Auto-installed modules',
  'page.industry.modulesSubtitle': 'Provisioned at tenant onboarding',
  'page.industry.none': 'None',
  'page.industry.standardsTitle': 'Default compliance standards',
  'page.industry.standardsSubtitle': 'Seeded into aether-compliance',
  'page.industry.kind.tracking': 'tracking',
  'page.industry.kind.quality': 'quality',
  'page.industry.kind.maintenance': 'maintenance',
  'page.industry.kind.safety': 'safety',
  'page.industry.kind.compliance': 'compliance',
  'page.industry.kind.process': 'process',
  'page.industry.kind.sustainability': 'sustainability',

  // Manager · work orders
  'page.wo.title': 'Work orders',
  'page.wo.new': 'New (PR #2)',
  'page.wo.queueTitle': 'Active queue',
  'page.wo.queueSubtitle': 'Realtime state arrives in PR #2 once the sync engine ships',
  'page.wo.col.code': 'Code',
  'page.wo.col.product': 'Product',
  'page.wo.col.quantity': 'Quantity',
  'page.wo.col.status': 'Status',
  'page.wo.open': 'Open',
  'page.wo.status.running': 'running',
  'page.wo.status.released': 'released',
  'page.wo.status.paused': 'paused',
  'page.wo.status.draft': 'draft',

  // Manager · machines
  'page.machines.title': 'Machines',
  'page.machines.note': 'Live OPC-UA / MQTT data wires up in PR #3.',
  'page.machines.status.running': 'running',
  'page.machines.status.idle': 'idle',
  'page.machines.status.fault': 'fault',
  'page.machines.status.maintenance': 'maintenance',

  // Manager · certifications
  'page.certs.title': 'Certifications & interlocks',
  'page.certs.intro':
    'Operators can only start a machine if every required certification is valid. The permit signal is wired directly to the safety relay — invalid certs mean the motor physically cannot energize.',
  'page.certs.matrixTitle': 'Operator × machine matrix',
  'page.certs.col.operator': 'Operator',
  'page.certs.status.expired': 'expired',
  'page.certs.status.missing': 'missing',

  // Manager · AI support
  'page.support.title': 'AI support',
  'page.support.intro':
    "When something goes wrong, you don't need to call us. AETHER-OS diagnoses, fixes what it safely can, and explains the rest in plain language. Anything that needs your approval shows up here with a single button.",
  'page.support.urgency.ok': 'auto-fixed',
  'page.support.urgency.watch': 'watching',
  'page.support.urgency.needs-you': 'needs you',
  'page.support.approved': 'approved · running now',
  'page.support.approveBtn': 'Approve auto-fix',
  'page.support.talkToHuman': 'Talk to a human (PR #5)',
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

  'page.overview.title': 'Overzicht',
  'page.overview.kpi.oee': 'OEE (totaal)',
  'page.overview.kpi.throughput': 'Doorvoer (stuks/u)',
  'page.overview.kpi.quality': 'Kwaliteit eerste keer goed',
  'page.overview.kpi.margin': 'Marge (voortschrijdend 30d)',
  'page.overview.hint.telemetry': 'gekoppeld in PR #3',
  'page.overview.hint.finance': 'financiële feed in PR #5',

  'page.digitalTwin.title': 'Digitale tweeling',
  'page.digitalTwin.viewerTitle': 'Fabrieksviewer',
  'page.digitalTwin.viewerSubtitle': 'React Three Fiber-scène wordt hier gekoppeld in PR #6',
  'page.digitalTwin.placeholder':
    'Tijdelijke aanduiding · 3D-model + live telemetrie-overlay komt in PR #6',

  'page.compliance.title': 'Nalevingsattest',
  'page.compliance.intro':
    'Live status van elke wettelijke + vrijwillige norm waarvoor je bent ingeschreven. Elk rapport wordt ondertekend door de AETHER-attestatieservice zodat externe auditors het kunnen verifiëren zonder ons te vertrouwen.',
  'page.compliance.rollup.enrolled': 'Ingeschreven',
  'page.compliance.rollup.compliant': 'Conform',
  'page.compliance.rollup.needsReview': 'Beoordeling nodig',
  'page.compliance.rollup.nonCompliant': 'Niet-conform',
  'page.compliance.standardsTitle': 'Normen',
  'page.compliance.standardsSubtitle':
    'Demogegevens — echte probes worden gekoppeld in PR #6 samen met de attestatie-ondertekeningssleutel.',
  'page.compliance.col.standard': 'Norm',
  'page.compliance.col.jurisdiction': 'Rechtsgebied',
  'page.compliance.col.kind': 'Soort',
  'page.compliance.col.controls': 'Controles',
  'page.compliance.col.status': 'Status',
  'page.compliance.col.lastReport': 'Laatste rapport',
  'page.compliance.status.compliant': 'Conform',
  'page.compliance.status.needs-review': 'Beoordeling nodig',
  'page.compliance.status.non-compliant': 'Niet-conform',
  'page.compliance.status.not-run': 'Niet uitgevoerd',
  'page.compliance.kind.international': 'Internationaal',
  'page.compliance.kind.industry': 'Branche',
  'page.compliance.kind.regulation': 'Regelgeving',
  'page.compliance.kind.national': 'Nationaal',

  'page.infra.title': 'Infrastructuur',
  'page.infra.note':
    'Service-statusmetrieken verschijnen in PR #2 zodra aether-telemetry in de Tauri-runtime is gekoppeld.',

  'page.discovery.title': 'IoT-autodetectie',
  'page.discovery.intro':
    'Scan het lokale netwerk op OPC-UA-, MQTT-, Modbus- en EtherNet/IP-apparaten. Voorgestelde tagkoppelingen verschijnen naast elke match.',
  'page.discovery.scan': 'Netwerk scannen',
  'page.discovery.scanning': 'Bezig met scannen…',
  'page.discovery.scanningNote':
    'Sonderen van /24 met 64-voudige gelijktijdigheid · 4 protocolprobes · 1,5 s/host time-out',
  'page.discovery.emptyNote':
    'Geen scan bezig. Het skelet toont demoresultaten wanneer je op “Netwerk scannen” klikt. Echt sonderen wordt gekoppeld in PR #3 samen met de protocolbruggen.',
  'page.discovery.bindings': '{count} voorgestelde tagkoppelingen · vingerafdruk',
  'page.discovery.addToInventory': 'Aan voorraad toevoegen (PR #3)',

  'page.health.title': 'Systeemstatus',
  'page.health.intro':
    'Synchronisatievertraging, outbox-diepte, telemetriedoorvoer, plus het logboek van Neural Auto-Healing.',
  'page.health.kpi.outbox': 'Outbox-diepte',
  'page.health.kpi.syncLag': 'Synchronisatievertraging (p99)',
  'page.health.kpi.telemetryRate': 'Telemetriesnelheid',
  'page.health.kpi.diskFree': 'Vrije schijfruimte',
  'page.health.kpiHint': 'gekoppeld in PR #3',
  'page.health.ledgerTitle': 'Herstellogboek',
  'page.health.ledgerSubtitle':
    'Append-only audit van elke automatische herstelactie. Demogegevens getoond.',
  'page.health.col.when': 'Wanneer',
  'page.health.col.source': 'Bron',
  'page.health.col.symptom': 'Symptoom',
  'page.health.col.policy': 'Beleid',
  'page.health.col.outcome': 'Resultaat',
  'page.health.outcome.applied': 'toegepast',
  'page.health.outcome.skipped': 'overgeslagen',
  'page.health.outcome.failed': 'mislukt',

  'page.industry.title': 'Sectorprofiel',
  'page.industry.intro':
    'Kies de sector van de tenant. AETHER-OS activeert automatisch mogelijkheden, standaardmodules en nalevingsnormen — geen aparte versie per klant. 21 branches ondersteund. De keuze hieronder stuurt de live CapabilityProvider aan; ga naar Manager → Voorraad om de kolommen te zien hervormen.',
  'page.industry.more': '+ 18 meer (PR #6)',
  'page.industry.capsSuffix': '{count} automatische mogelijkheden',
  'page.industry.capsSubtitle':
    'Automatisch geactiveerd wanneer dit profiel op de tenant is ingesteld.',
  'page.industry.modulesTitle': 'Automatisch geïnstalleerde modules',
  'page.industry.modulesSubtitle': 'Voorzien bij tenant-onboarding',
  'page.industry.none': 'Geen',
  'page.industry.standardsTitle': 'Standaard nalevingsnormen',
  'page.industry.standardsSubtitle': 'Geïnitialiseerd in aether-compliance',
  'page.industry.kind.tracking': 'tracering',
  'page.industry.kind.quality': 'kwaliteit',
  'page.industry.kind.maintenance': 'onderhoud',
  'page.industry.kind.safety': 'veiligheid',
  'page.industry.kind.compliance': 'naleving',
  'page.industry.kind.process': 'proces',
  'page.industry.kind.sustainability': 'duurzaamheid',

  'page.wo.title': 'Werkorders',
  'page.wo.new': 'Nieuw (PR #2)',
  'page.wo.queueTitle': 'Actieve wachtrij',
  'page.wo.queueSubtitle':
    'Realtime status verschijnt in PR #2 zodra de synchronisatie-engine wordt uitgebracht',
  'page.wo.col.code': 'Code',
  'page.wo.col.product': 'Product',
  'page.wo.col.quantity': 'Aantal',
  'page.wo.col.status': 'Status',
  'page.wo.open': 'Openen',
  'page.wo.status.running': 'actief',
  'page.wo.status.released': 'vrijgegeven',
  'page.wo.status.paused': 'gepauzeerd',
  'page.wo.status.draft': 'concept',

  'page.machines.title': 'Machines',
  'page.machines.note': 'Live OPC-UA-/MQTT-gegevens worden gekoppeld in PR #3.',
  'page.machines.status.running': 'actief',
  'page.machines.status.idle': 'inactief',
  'page.machines.status.fault': 'storing',
  'page.machines.status.maintenance': 'onderhoud',

  'page.certs.title': 'Certificeringen & vergrendelingen',
  'page.certs.intro':
    'Operators kunnen een machine alleen starten als elke vereiste certificering geldig is. Het toestemmingssignaal is rechtstreeks op het veiligheidsrelais aangesloten — ongeldige certificeringen betekenen dat de motor fysiek niet kan inschakelen.',
  'page.certs.matrixTitle': 'Operator × machine-matrix',
  'page.certs.col.operator': 'Operator',
  'page.certs.status.expired': 'verlopen',
  'page.certs.status.missing': 'ontbreekt',

  'page.support.title': 'AI-ondersteuning',
  'page.support.intro':
    'Als er iets misgaat, hoef je ons niet te bellen. AETHER-OS stelt een diagnose, lost veilig op wat het kan en legt de rest in gewone taal uit. Alles wat jouw goedkeuring nodig heeft, verschijnt hier met één knop.',
  'page.support.urgency.ok': 'automatisch opgelost',
  'page.support.urgency.watch': 'in de gaten',
  'page.support.urgency.needs-you': 'actie nodig',
  'page.support.approved': 'goedgekeurd · wordt nu uitgevoerd',
  'page.support.approveBtn': 'Auto-fix goedkeuren',
  'page.support.talkToHuman': 'Praat met een mens (PR #5)',
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

  'page.overview.title': 'Übersicht',
  'page.overview.kpi.oee': 'OEE (gesamt)',
  'page.overview.kpi.throughput': 'Durchsatz (Stück/h)',
  'page.overview.kpi.quality': 'Qualität Erstdurchlauf',
  'page.overview.kpi.margin': 'Marge (rollierend 30 T)',
  'page.overview.hint.telemetry': 'angebunden in PR #3',
  'page.overview.hint.finance': 'Finanz-Feed in PR #5',

  'page.digitalTwin.title': 'Digitaler Zwilling',
  'page.digitalTwin.viewerTitle': 'Werksansicht',
  'page.digitalTwin.viewerSubtitle': 'React-Three-Fiber-Szene wird hier in PR #6 eingebunden',
  'page.digitalTwin.placeholder':
    'Platzhalter · 3D-Modell + Live-Telemetrie-Overlay folgt in PR #6',

  'page.compliance.title': 'Compliance-Attestierung',
  'page.compliance.intro':
    'Live-Status für jede regulatorische + freiwillige Norm, für die Sie registriert sind. Jeder Bericht wird vom AETHER-Attestierungsdienst signiert, sodass externe Auditoren ihn überprüfen können, ohne uns vertrauen zu müssen.',
  'page.compliance.rollup.enrolled': 'Registriert',
  'page.compliance.rollup.compliant': 'Konform',
  'page.compliance.rollup.needsReview': 'Prüfung nötig',
  'page.compliance.rollup.nonCompliant': 'Nicht konform',
  'page.compliance.standardsTitle': 'Normen',
  'page.compliance.standardsSubtitle':
    'Demodaten — echte Probes werden in PR #6 zusammen mit dem Attestierungs-Signaturschlüssel angebunden.',
  'page.compliance.col.standard': 'Norm',
  'page.compliance.col.jurisdiction': 'Zuständigkeit',
  'page.compliance.col.kind': 'Art',
  'page.compliance.col.controls': 'Kontrollen',
  'page.compliance.col.status': 'Status',
  'page.compliance.col.lastReport': 'Letzter Bericht',
  'page.compliance.status.compliant': 'Konform',
  'page.compliance.status.needs-review': 'Prüfung nötig',
  'page.compliance.status.non-compliant': 'Nicht konform',
  'page.compliance.status.not-run': 'Nicht ausgeführt',
  'page.compliance.kind.international': 'International',
  'page.compliance.kind.industry': 'Branche',
  'page.compliance.kind.regulation': 'Regulierung',
  'page.compliance.kind.national': 'National',

  'page.infra.title': 'Infrastruktur',
  'page.infra.note':
    'Service-Zustandsmetriken erscheinen in PR #2, sobald aether-telemetry in die Tauri-Laufzeit eingebunden ist.',

  'page.discovery.title': 'IoT-Autoerkennung',
  'page.discovery.intro':
    'Durchsuchen Sie das lokale Netzwerk nach OPC-UA-, MQTT-, Modbus- und EtherNet/IP-Geräten. Vorgeschlagene Tag-Bindungen erscheinen neben jedem Treffer.',
  'page.discovery.scan': 'Netzwerk scannen',
  'page.discovery.scanning': 'Scannen…',
  'page.discovery.scanningNote':
    'Sondierung /24 mit 64-facher Nebenläufigkeit · 4 Protokoll-Probes · 1,5 s/Host-Timeout',
  'page.discovery.emptyNote':
    'Kein Scan aktiv. Das Gerüst zeigt Demo-Ergebnisse, wenn Sie auf „Netzwerk scannen“ klicken. Echte Sondierung wird in PR #3 zusammen mit den Protokollbrücken eingebunden.',
  'page.discovery.bindings': '{count} vorgeschlagene Tag-Bindungen · Fingerabdruck',
  'page.discovery.addToInventory': 'Zum Bestand hinzufügen (PR #3)',

  'page.health.title': 'Systemzustand',
  'page.health.intro':
    'Synchronisierungsverzögerung, Outbox-Tiefe, Telemetriedurchsatz sowie das Ereignisprotokoll der neuronalen Selbstheilung.',
  'page.health.kpi.outbox': 'Outbox-Tiefe',
  'page.health.kpi.syncLag': 'Synchronisierungsverzögerung (p99)',
  'page.health.kpi.telemetryRate': 'Telemetrierate',
  'page.health.kpi.diskFree': 'Freier Speicher',
  'page.health.kpiHint': 'angebunden in PR #3',
  'page.health.ledgerTitle': 'Heilungsprotokoll',
  'page.health.ledgerSubtitle':
    'Append-only-Audit jeder automatischen Behebung. Demodaten gezeigt.',
  'page.health.col.when': 'Wann',
  'page.health.col.source': 'Quelle',
  'page.health.col.symptom': 'Symptom',
  'page.health.col.policy': 'Richtlinie',
  'page.health.col.outcome': 'Ergebnis',
  'page.health.outcome.applied': 'angewendet',
  'page.health.outcome.skipped': 'übersprungen',
  'page.health.outcome.failed': 'fehlgeschlagen',

  'page.industry.title': 'Branchenprofil',
  'page.industry.intro':
    'Wählen Sie die Branche des Mandanten. AETHER-OS aktiviert automatisch Funktionen, Standardmodule und Compliance-Normen — kein Fork pro Kunde. 21 Branchen unterstützt. Die Auswahl unten steuert den Live-CapabilityProvider; wechseln Sie zu Manager → Bestand, um die Spalten umformen zu sehen.',
  'page.industry.more': '+ 18 weitere (PR #6)',
  'page.industry.capsSuffix': '{count} Auto-Funktionen',
  'page.industry.capsSubtitle':
    'Automatisch aktiviert, wenn dieses Profil für den Mandanten gesetzt ist.',
  'page.industry.modulesTitle': 'Automatisch installierte Module',
  'page.industry.modulesSubtitle': 'Bereitgestellt beim Mandanten-Onboarding',
  'page.industry.none': 'Keine',
  'page.industry.standardsTitle': 'Standard-Compliance-Normen',
  'page.industry.standardsSubtitle': 'In aether-compliance initialisiert',
  'page.industry.kind.tracking': 'Verfolgung',
  'page.industry.kind.quality': 'Qualität',
  'page.industry.kind.maintenance': 'Wartung',
  'page.industry.kind.safety': 'Sicherheit',
  'page.industry.kind.compliance': 'Compliance',
  'page.industry.kind.process': 'Prozess',
  'page.industry.kind.sustainability': 'Nachhaltigkeit',

  'page.wo.title': 'Arbeitsaufträge',
  'page.wo.new': 'Neu (PR #2)',
  'page.wo.queueTitle': 'Aktive Warteschlange',
  'page.wo.queueSubtitle':
    'Echtzeitstatus erscheint in PR #2, sobald die Synchronisierungs-Engine ausgeliefert wird',
  'page.wo.col.code': 'Code',
  'page.wo.col.product': 'Produkt',
  'page.wo.col.quantity': 'Menge',
  'page.wo.col.status': 'Status',
  'page.wo.open': 'Öffnen',
  'page.wo.status.running': 'läuft',
  'page.wo.status.released': 'freigegeben',
  'page.wo.status.paused': 'pausiert',
  'page.wo.status.draft': 'Entwurf',

  'page.machines.title': 'Maschinen',
  'page.machines.note': 'Live-OPC-UA-/MQTT-Daten werden in PR #3 angebunden.',
  'page.machines.status.running': 'läuft',
  'page.machines.status.idle': 'inaktiv',
  'page.machines.status.fault': 'Störung',
  'page.machines.status.maintenance': 'Wartung',

  'page.certs.title': 'Zertifizierungen & Verriegelungen',
  'page.certs.intro':
    'Operatoren können eine Maschine nur starten, wenn jede erforderliche Zertifizierung gültig ist. Das Freigabesignal ist direkt mit dem Sicherheitsrelais verbunden — ungültige Zertifizierungen bedeuten, dass der Motor physisch nicht anlaufen kann.',
  'page.certs.matrixTitle': 'Operator-×-Maschine-Matrix',
  'page.certs.col.operator': 'Operator',
  'page.certs.status.expired': 'abgelaufen',
  'page.certs.status.missing': 'fehlt',

  'page.support.title': 'KI-Support',
  'page.support.intro':
    'Wenn etwas schiefgeht, müssen Sie uns nicht anrufen. AETHER-OS stellt eine Diagnose, behebt sicher, was möglich ist, und erklärt den Rest in einfacher Sprache. Alles, was Ihre Freigabe erfordert, erscheint hier mit einer einzigen Schaltfläche.',
  'page.support.urgency.ok': 'automatisch behoben',
  'page.support.urgency.watch': 'wird beobachtet',
  'page.support.urgency.needs-you': 'Aktion nötig',
  'page.support.approved': 'freigegeben · läuft jetzt',
  'page.support.approveBtn': 'Auto-Fix freigeben',
  'page.support.talkToHuman': 'Mit einem Menschen sprechen (PR #5)',
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

  'page.overview.title': 'Ikhtisar',
  'page.overview.kpi.oee': 'OEE (keseluruhan)',
  'page.overview.kpi.throughput': 'Hasil (unit/jam)',
  'page.overview.kpi.quality': 'Kualitas lolos pertama',
  'page.overview.kpi.margin': 'Margin (30 hari berjalan)',
  'page.overview.hint.telemetry': 'dihubungkan di PR #3',
  'page.overview.hint.finance': 'umpan keuangan di PR #5',

  'page.digitalTwin.title': 'Kembar digital',
  'page.digitalTwin.viewerTitle': 'Penampil pabrik',
  'page.digitalTwin.viewerSubtitle': 'Adegan React Three Fiber dipasang di sini pada PR #6',
  'page.digitalTwin.placeholder':
    'Placeholder · model 3D + overlay telemetri langsung hadir di PR #6',

  'page.compliance.title': 'Atestasi kepatuhan',
  'page.compliance.intro':
    'Status langsung di seluruh standar regulasi + sukarela yang Anda ikuti. Setiap laporan ditandatangani oleh layanan atestasi AETHER sehingga auditor eksternal dapat memverifikasinya tanpa harus memercayai kami.',
  'page.compliance.rollup.enrolled': 'Terdaftar',
  'page.compliance.rollup.compliant': 'Patuh',
  'page.compliance.rollup.needsReview': 'Perlu tinjauan',
  'page.compliance.rollup.nonCompliant': 'Tidak patuh',
  'page.compliance.standardsTitle': 'Standar',
  'page.compliance.standardsSubtitle':
    'Data demo — probe sungguhan dihubungkan di PR #6 bersama kunci penandatanganan atestasi.',
  'page.compliance.col.standard': 'Standar',
  'page.compliance.col.jurisdiction': 'Yurisdiksi',
  'page.compliance.col.kind': 'Jenis',
  'page.compliance.col.controls': 'Kontrol',
  'page.compliance.col.status': 'Status',
  'page.compliance.col.lastReport': 'Laporan terakhir',
  'page.compliance.status.compliant': 'Patuh',
  'page.compliance.status.needs-review': 'Perlu tinjauan',
  'page.compliance.status.non-compliant': 'Tidak patuh',
  'page.compliance.status.not-run': 'Belum dijalankan',
  'page.compliance.kind.international': 'Internasional',
  'page.compliance.kind.industry': 'Industri',
  'page.compliance.kind.regulation': 'Regulasi',
  'page.compliance.kind.national': 'Nasional',

  'page.infra.title': 'Infrastruktur',
  'page.infra.note':
    'Metrik kesehatan layanan hadir di PR #2 setelah aether-telemetry terhubung ke runtime Tauri.',

  'page.discovery.title': 'Penemuan Otomatis IoT',
  'page.discovery.intro':
    'Pindai jaringan lokal untuk perangkat OPC-UA, MQTT, Modbus, dan EtherNet/IP. Saran pengikatan tag muncul di samping setiap kecocokan.',
  'page.discovery.scan': 'Pindai jaringan',
  'page.discovery.scanning': 'Memindai…',
  'page.discovery.scanningNote':
    'Menyelidiki /24 dengan konkurensi 64 arah · 4 probe protokol · batas waktu 1,5 dtk/host',
  'page.discovery.emptyNote':
    'Tidak ada pemindaian berlangsung. Kerangka menampilkan hasil demo saat Anda mengklik “Pindai jaringan”. Penyelidikan sungguhan dihubungkan di PR #3 bersama jembatan protokol.',
  'page.discovery.bindings': '{count} saran pengikatan tag · sidik jari',
  'page.discovery.addToInventory': 'Tambahkan ke inventaris (PR #3)',

  'page.health.title': 'Kesehatan sistem',
  'page.health.intro':
    'Jeda sinkronisasi, kedalaman outbox, throughput telemetri, plus buku besar peristiwa Neural Auto-Healing.',
  'page.health.kpi.outbox': 'Kedalaman outbox',
  'page.health.kpi.syncLag': 'Jeda sinkronisasi (p99)',
  'page.health.kpi.telemetryRate': 'Laju telemetri',
  'page.health.kpi.diskFree': 'Ruang disk bebas',
  'page.health.kpiHint': 'dihubungkan di PR #3',
  'page.health.ledgerTitle': 'Buku besar penyembuhan',
  'page.health.ledgerSubtitle':
    'Audit hanya-tambah untuk setiap remediasi otomatis. Data demo ditampilkan.',
  'page.health.col.when': 'Kapan',
  'page.health.col.source': 'Sumber',
  'page.health.col.symptom': 'Gejala',
  'page.health.col.policy': 'Kebijakan',
  'page.health.col.outcome': 'Hasil',
  'page.health.outcome.applied': 'diterapkan',
  'page.health.outcome.skipped': 'dilewati',
  'page.health.outcome.failed': 'gagal',

  'page.industry.title': 'Profil industri',
  'page.industry.intro':
    'Pilih industri tenant. AETHER-OS otomatis mengaktifkan kapabilitas, modul default, dan standar kepatuhan — tanpa fork per pelanggan. 21 vertikal didukung. Pemilih di bawah menggerakkan CapabilityProvider langsung; beralih ke Manajer → Inventaris untuk melihat kolom berubah bentuk.',
  'page.industry.more': '+ 18 lainnya (PR #6)',
  'page.industry.capsSuffix': '{count} kapabilitas otomatis',
  'page.industry.capsSubtitle': 'Diaktifkan otomatis saat profil ini disetel pada tenant.',
  'page.industry.modulesTitle': 'Modul terpasang otomatis',
  'page.industry.modulesSubtitle': 'Disediakan saat onboarding tenant',
  'page.industry.none': 'Tidak ada',
  'page.industry.standardsTitle': 'Standar kepatuhan default',
  'page.industry.standardsSubtitle': 'Ditanamkan ke aether-compliance',
  'page.industry.kind.tracking': 'pelacakan',
  'page.industry.kind.quality': 'kualitas',
  'page.industry.kind.maintenance': 'pemeliharaan',
  'page.industry.kind.safety': 'keselamatan',
  'page.industry.kind.compliance': 'kepatuhan',
  'page.industry.kind.process': 'proses',
  'page.industry.kind.sustainability': 'keberlanjutan',

  'page.wo.title': 'Perintah kerja',
  'page.wo.new': 'Baru (PR #2)',
  'page.wo.queueTitle': 'Antrean aktif',
  'page.wo.queueSubtitle': 'Status realtime hadir di PR #2 setelah mesin sinkronisasi dirilis',
  'page.wo.col.code': 'Kode',
  'page.wo.col.product': 'Produk',
  'page.wo.col.quantity': 'Jumlah',
  'page.wo.col.status': 'Status',
  'page.wo.open': 'Buka',
  'page.wo.status.running': 'berjalan',
  'page.wo.status.released': 'dirilis',
  'page.wo.status.paused': 'dijeda',
  'page.wo.status.draft': 'draf',

  'page.machines.title': 'Mesin',
  'page.machines.note': 'Data OPC-UA / MQTT langsung dihubungkan di PR #3.',
  'page.machines.status.running': 'berjalan',
  'page.machines.status.idle': 'menganggur',
  'page.machines.status.fault': 'gangguan',
  'page.machines.status.maintenance': 'pemeliharaan',

  'page.certs.title': 'Sertifikasi & interlock',
  'page.certs.intro':
    'Operator hanya dapat menjalankan mesin jika setiap sertifikasi yang diperlukan valid. Sinyal izin terhubung langsung ke relai keselamatan — sertifikasi tidak valid berarti motor secara fisik tidak dapat menyala.',
  'page.certs.matrixTitle': 'Matriks operator × mesin',
  'page.certs.col.operator': 'Operator',
  'page.certs.status.expired': 'kedaluwarsa',
  'page.certs.status.missing': 'tidak ada',

  'page.support.title': 'Dukungan AI',
  'page.support.intro':
    'Saat terjadi masalah, Anda tidak perlu menghubungi kami. AETHER-OS mendiagnosis, memperbaiki yang aman untuk diperbaiki, dan menjelaskan sisanya dalam bahasa sederhana. Apa pun yang membutuhkan persetujuan Anda muncul di sini dengan satu tombol.',
  'page.support.urgency.ok': 'diperbaiki otomatis',
  'page.support.urgency.watch': 'memantau',
  'page.support.urgency.needs-you': 'perlu Anda',
  'page.support.approved': 'disetujui · berjalan sekarang',
  'page.support.approveBtn': 'Setujui perbaikan otomatis',
  'page.support.talkToHuman': 'Bicara dengan manusia (PR #5)',
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

  'page.overview.title': '概要',
  'page.overview.kpi.oee': 'OEE（総合）',
  'page.overview.kpi.throughput': 'スループット（個/時）',
  'page.overview.kpi.quality': '一発良品率',
  'page.overview.kpi.margin': '利益率（30日移動）',
  'page.overview.hint.telemetry': 'PR #3で接続',
  'page.overview.hint.finance': 'PR #5で財務フィード',

  'page.digitalTwin.title': 'デジタルツイン',
  'page.digitalTwin.viewerTitle': 'プラントビューア',
  'page.digitalTwin.viewerSubtitle': 'React Three FiberシーンはPR #6でここに組み込まれます',
  'page.digitalTwin.placeholder':
    'プレースホルダー · 3Dモデル + ライブテレメトリオーバーレイはPR #6で対応',

  'page.compliance.title': 'コンプライアンス認証',
  'page.compliance.intro':
    '登録済みのすべての規制・任意規格にわたるライブステータス。各レポートはAETHER認証サービスによって署名されるため、外部監査人は当社を信頼することなく検証できます。',
  'page.compliance.rollup.enrolled': '登録済み',
  'page.compliance.rollup.compliant': '適合',
  'page.compliance.rollup.needsReview': '要確認',
  'page.compliance.rollup.nonCompliant': '不適合',
  'page.compliance.standardsTitle': '規格',
  'page.compliance.standardsSubtitle':
    'デモデータ — 実際のプローブはPR #6で認証署名鍵とともに接続されます。',
  'page.compliance.col.standard': '規格',
  'page.compliance.col.jurisdiction': '管轄',
  'page.compliance.col.kind': '種類',
  'page.compliance.col.controls': '統制',
  'page.compliance.col.status': 'ステータス',
  'page.compliance.col.lastReport': '最新レポート',
  'page.compliance.status.compliant': '適合',
  'page.compliance.status.needs-review': '要確認',
  'page.compliance.status.non-compliant': '不適合',
  'page.compliance.status.not-run': '未実行',
  'page.compliance.kind.international': '国際',
  'page.compliance.kind.industry': '業界',
  'page.compliance.kind.regulation': '規制',
  'page.compliance.kind.national': '国内',

  'page.infra.title': 'インフラ',
  'page.infra.note':
    'サービス正常性メトリクスは、aether-telemetryがTauriランタイムに組み込まれるPR #2で表示されます。',

  'page.discovery.title': 'IoT自動検出',
  'page.discovery.intro':
    'ローカルネットワークでOPC-UA、MQTT、Modbus、EtherNet/IPデバイスをスキャンします。各一致の横に推奨タグバインディングが表示されます。',
  'page.discovery.scan': 'ネットワークをスキャン',
  'page.discovery.scanning': 'スキャン中…',
  'page.discovery.scanningNote':
    '/24を64並列でプローブ · 4つのプロトコルプローブ · ホストあたり1.5秒タイムアウト',
  'page.discovery.emptyNote':
    'スキャンは実行されていません。「ネットワークをスキャン」をクリックすると、スケルトンはデモ結果を表示します。実際のプローブはPR #3でプロトコルブリッジとともに接続されます。',
  'page.discovery.bindings': '推奨タグバインディング {count} 件 · フィンガープリント',
  'page.discovery.addToInventory': '在庫に追加（PR #3）',

  'page.health.title': 'システム正常性',
  'page.health.intro':
    '同期遅延、outbox深度、テレメトリスループット、さらにNeural Auto-Healingのイベント台帳。',
  'page.health.kpi.outbox': 'Outbox深度',
  'page.health.kpi.syncLag': '同期遅延（p99）',
  'page.health.kpi.telemetryRate': 'テレメトリ速度',
  'page.health.kpi.diskFree': '空きディスク',
  'page.health.kpiHint': 'PR #3で接続',
  'page.health.ledgerTitle': 'ヒーリング台帳',
  'page.health.ledgerSubtitle': 'すべての自動修復の追記専用監査。デモデータを表示。',
  'page.health.col.when': '日時',
  'page.health.col.source': 'ソース',
  'page.health.col.symptom': '症状',
  'page.health.col.policy': 'ポリシー',
  'page.health.col.outcome': '結果',
  'page.health.outcome.applied': '適用',
  'page.health.outcome.skipped': 'スキップ',
  'page.health.outcome.failed': '失敗',

  'page.industry.title': '業種プロファイル',
  'page.industry.intro':
    'テナントの業種を選択します。AETHER-OSは機能、デフォルトモジュール、コンプライアンス規格を自動的に有効化します — 顧客ごとのフォークは不要。21業種に対応。下のピッカーはライブのCapabilityProviderを制御します。マネージャー → 在庫に切り替えると列が再構成されるのを確認できます。',
  'page.industry.more': '+ 他18件（PR #6）',
  'page.industry.capsSuffix': '自動機能 {count} 件',
  'page.industry.capsSubtitle': 'このプロファイルがテナントに設定されると自動的に有効化されます。',
  'page.industry.modulesTitle': '自動インストールされるモジュール',
  'page.industry.modulesSubtitle': 'テナントのオンボーディング時にプロビジョニング',
  'page.industry.none': 'なし',
  'page.industry.standardsTitle': 'デフォルトのコンプライアンス規格',
  'page.industry.standardsSubtitle': 'aether-complianceに初期投入',
  'page.industry.kind.tracking': 'トラッキング',
  'page.industry.kind.quality': '品質',
  'page.industry.kind.maintenance': '保全',
  'page.industry.kind.safety': '安全',
  'page.industry.kind.compliance': 'コンプライアンス',
  'page.industry.kind.process': 'プロセス',
  'page.industry.kind.sustainability': 'サステナビリティ',

  'page.wo.title': '作業指示',
  'page.wo.new': '新規（PR #2）',
  'page.wo.queueTitle': 'アクティブキュー',
  'page.wo.queueSubtitle': 'リアルタイム状態は、同期エンジンが出荷されるPR #2で表示されます',
  'page.wo.col.code': 'コード',
  'page.wo.col.product': '製品',
  'page.wo.col.quantity': '数量',
  'page.wo.col.status': 'ステータス',
  'page.wo.open': '開く',
  'page.wo.status.running': '稼働中',
  'page.wo.status.released': 'リリース済み',
  'page.wo.status.paused': '一時停止',
  'page.wo.status.draft': '下書き',

  'page.machines.title': '設備',
  'page.machines.note': 'ライブのOPC-UA / MQTTデータはPR #3で接続されます。',
  'page.machines.status.running': '稼働中',
  'page.machines.status.idle': '待機',
  'page.machines.status.fault': '故障',
  'page.machines.status.maintenance': '保全',

  'page.certs.title': '認定 & インターロック',
  'page.certs.intro':
    '必要なすべての認定が有効な場合にのみ、オペレーターは設備を起動できます。許可信号は安全リレーに直接接続されており、無効な認定ではモーターは物理的に起動できません。',
  'page.certs.matrixTitle': 'オペレーター × 設備マトリクス',
  'page.certs.col.operator': 'オペレーター',
  'page.certs.status.expired': '期限切れ',
  'page.certs.status.missing': '未取得',

  'page.support.title': 'AIサポート',
  'page.support.intro':
    '問題が発生しても、当社に電話する必要はありません。AETHER-OSが診断し、安全に対処できるものは修正し、残りを平易な言葉で説明します。承認が必要なものは、ボタン1つでここに表示されます。',
  'page.support.urgency.ok': '自動修正済み',
  'page.support.urgency.watch': '監視中',
  'page.support.urgency.needs-you': '要対応',
  'page.support.approved': '承認済み · 実行中',
  'page.support.approveBtn': '自動修正を承認',
  'page.support.talkToHuman': '担当者と話す（PR #5）',
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

  'page.overview.title': '概览',
  'page.overview.kpi.oee': 'OEE（综合）',
  'page.overview.kpi.throughput': '产量（件/小时）',
  'page.overview.kpi.quality': '一次合格率',
  'page.overview.kpi.margin': '利润率（滚动 30 天）',
  'page.overview.hint.telemetry': '在 PR #3 接入',
  'page.overview.hint.finance': '在 PR #5 接入财务数据',

  'page.digitalTwin.title': '数字孪生',
  'page.digitalTwin.viewerTitle': '工厂查看器',
  'page.digitalTwin.viewerSubtitle': 'React Three Fiber 场景将在 PR #6 接入此处',
  'page.digitalTwin.placeholder': '占位符 · 3D 模型 + 实时遥测叠加将在 PR #6 接入',

  'page.compliance.title': '合规认证',
  'page.compliance.intro':
    '您已注册的每项法规及自愿标准的实时状态。每份报告均由 AETHER 认证服务签名，外部审计员无需信任我们即可验证。',
  'page.compliance.rollup.enrolled': '已注册',
  'page.compliance.rollup.compliant': '合规',
  'page.compliance.rollup.needsReview': '需复核',
  'page.compliance.rollup.nonCompliant': '不合规',
  'page.compliance.standardsTitle': '标准',
  'page.compliance.standardsSubtitle': '演示数据 — 真实探针将在 PR #6 与认证签名密钥一起接入。',
  'page.compliance.col.standard': '标准',
  'page.compliance.col.jurisdiction': '司法管辖区',
  'page.compliance.col.kind': '类别',
  'page.compliance.col.controls': '控制项',
  'page.compliance.col.status': '状态',
  'page.compliance.col.lastReport': '最新报告',
  'page.compliance.status.compliant': '合规',
  'page.compliance.status.needs-review': '需复核',
  'page.compliance.status.non-compliant': '不合规',
  'page.compliance.status.not-run': '未运行',
  'page.compliance.kind.international': '国际',
  'page.compliance.kind.industry': '行业',
  'page.compliance.kind.regulation': '法规',
  'page.compliance.kind.national': '国家',

  'page.infra.title': '基础设施',
  'page.infra.note': '服务健康指标将在 PR #2 中显示，届时 aether-telemetry 已接入 Tauri 运行时。',

  'page.discovery.title': 'IoT 自动发现',
  'page.discovery.intro':
    '扫描本地网络以查找 OPC-UA、MQTT、Modbus 和 EtherNet/IP 设备。建议的标签绑定显示在每个匹配项旁边。',
  'page.discovery.scan': '扫描网络',
  'page.discovery.scanning': '扫描中…',
  'page.discovery.scanningNote': '以 64 路并发探测 /24 · 4 个协议探针 · 每主机 1.5 秒超时',
  'page.discovery.emptyNote':
    '没有正在进行的扫描。点击“扫描网络”时骨架会显示演示结果。真实探测将在 PR #3 与协议网桥一起接入。',
  'page.discovery.bindings': '{count} 个建议标签绑定 · 指纹',
  'page.discovery.addToInventory': '添加到库存（PR #3）',

  'page.health.title': '系统健康',
  'page.health.intro': '同步延迟、outbox 深度、遥测吞吐量，以及神经自愈事件账本。',
  'page.health.kpi.outbox': 'Outbox 深度',
  'page.health.kpi.syncLag': '同步延迟（p99）',
  'page.health.kpi.telemetryRate': '遥测速率',
  'page.health.kpi.diskFree': '可用磁盘',
  'page.health.kpiHint': '在 PR #3 接入',
  'page.health.ledgerTitle': '自愈账本',
  'page.health.ledgerSubtitle': '每次自动修复的仅追加审计。显示演示数据。',
  'page.health.col.when': '时间',
  'page.health.col.source': '来源',
  'page.health.col.symptom': '症状',
  'page.health.col.policy': '策略',
  'page.health.col.outcome': '结果',
  'page.health.outcome.applied': '已应用',
  'page.health.outcome.skipped': '已跳过',
  'page.health.outcome.failed': '失败',

  'page.industry.title': '行业配置',
  'page.industry.intro':
    '选择租户的行业。AETHER-OS 会自动激活功能、默认模块和合规标准 — 无需为每个客户分叉。支持 21 个垂直行业。下方选择器驱动实时 CapabilityProvider；切换到 经理 → 库存 即可看到列重新排布。',
  'page.industry.more': '+ 还有 18 个（PR #6）',
  'page.industry.capsSuffix': '{count} 项自动功能',
  'page.industry.capsSubtitle': '在为租户设置此配置时自动激活。',
  'page.industry.modulesTitle': '自动安装的模块',
  'page.industry.modulesSubtitle': '在租户引导时预置',
  'page.industry.none': '无',
  'page.industry.standardsTitle': '默认合规标准',
  'page.industry.standardsSubtitle': '已注入 aether-compliance',
  'page.industry.kind.tracking': '追踪',
  'page.industry.kind.quality': '质量',
  'page.industry.kind.maintenance': '维护',
  'page.industry.kind.safety': '安全',
  'page.industry.kind.compliance': '合规',
  'page.industry.kind.process': '工艺',
  'page.industry.kind.sustainability': '可持续性',

  'page.wo.title': '工单',
  'page.wo.new': '新建（PR #2）',
  'page.wo.queueTitle': '活动队列',
  'page.wo.queueSubtitle': '实时状态将在同步引擎发布的 PR #2 中显示',
  'page.wo.col.code': '编号',
  'page.wo.col.product': '产品',
  'page.wo.col.quantity': '数量',
  'page.wo.col.status': '状态',
  'page.wo.open': '打开',
  'page.wo.status.running': '运行中',
  'page.wo.status.released': '已下达',
  'page.wo.status.paused': '已暂停',
  'page.wo.status.draft': '草稿',

  'page.machines.title': '设备',
  'page.machines.note': '实时 OPC-UA / MQTT 数据将在 PR #3 接入。',
  'page.machines.status.running': '运行中',
  'page.machines.status.idle': '空闲',
  'page.machines.status.fault': '故障',
  'page.machines.status.maintenance': '维护',

  'page.certs.title': '认证与联锁',
  'page.certs.intro':
    '只有在每项必需认证均有效时，操作员才能启动设备。许可信号直接连接到安全继电器 — 认证无效意味着电机在物理上无法通电。',
  'page.certs.matrixTitle': '操作员 × 设备矩阵',
  'page.certs.col.operator': '操作员',
  'page.certs.status.expired': '已过期',
  'page.certs.status.missing': '缺失',

  'page.support.title': 'AI 支持',
  'page.support.intro':
    '出现问题时，您无需联系我们。AETHER-OS 会进行诊断，安全地修复力所能及的问题，并用通俗语言解释其余部分。任何需要您批准的事项都会在此通过一个按钮显示。',
  'page.support.urgency.ok': '已自动修复',
  'page.support.urgency.watch': '监视中',
  'page.support.urgency.needs-you': '需要您处理',
  'page.support.approved': '已批准 · 正在运行',
  'page.support.approveBtn': '批准自动修复',
  'page.support.talkToHuman': '联系人工（PR #5）',
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
