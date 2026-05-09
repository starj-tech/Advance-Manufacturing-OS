import { useEffect, useState } from 'react';
import { Card, CardBody, CardHeader, Stack, StatusPill, Button } from '@aether/ui-kit';
import { useIndustry, useSetIndustry } from '@aether/shell-runtime';

interface Capability {
  id: string;
  kind: 'tracking' | 'quality' | 'maintenance' | 'safety' | 'compliance' | 'process' | 'sustainability';
  display: string;
  description: string;
}

interface IndustryEntry {
  slug: string;
  display: string;
  capabilities: Capability[];
  defaultStandards: string[];
  autoModules: string[];
}

// Minimal demo subset rendered while Tauri command wiring is mocked.
// Real data comes from `industry_profile` IPC call (PR #6).
const PROFILES: Record<string, IndustryEntry> = {
  'food-and-beverage': {
    slug: 'food-and-beverage',
    display: 'Food & Beverage',
    capabilities: [
      { id: 'expired-date-tracking', kind: 'tracking', display: 'Expiry date tracking', description: 'Lot/batch expiry, FEFO picking, recall tracing' },
      { id: 'temperature-sensor-integration', kind: 'process', display: 'Temperature sensor integration', description: 'Auto-bind OPC-UA / Modbus temperature tags to material lots' },
      { id: 'cold-chain-monitor', kind: 'tracking', display: 'Cold chain monitoring', description: 'Continuous temperature & humidity logging with alarms' },
      { id: 'haccp-checks', kind: 'quality', display: 'HACCP control points', description: 'Critical control point checklists with corrective actions' },
      { id: 'recall-readiness', kind: 'compliance', display: 'Recall readiness', description: 'Trace any unit forwards/backwards in <2 minutes' },
    ],
    defaultStandards: ['ISO 22000', 'HACCP', 'FSSC 22000', 'BPOM'],
    autoModules: ['com.aether.cold-chain', 'com.aether.haccp'],
  },
  automotive: {
    slug: 'automotive',
    display: 'Automotive',
    capabilities: [
      { id: 'parts-serial-tracking', kind: 'tracking', display: 'Parts serial tracking', description: 'Per-component VIN linkage, supplier→assembly traceability' },
      { id: 'precision-calibration', kind: 'quality', display: 'Precision calibration', description: 'Tool wear, gauge R&R, automatic recalibration triggers' },
      { id: 'ppap-control-plan', kind: 'quality', display: 'PPAP & control plan', description: 'Production Part Approval Process documentation pack' },
      { id: 'andon-line-stop', kind: 'safety', display: 'Andon line-stop', description: 'Operator-initiated line stop tied to interlock' },
    ],
    defaultStandards: ['IATF 16949', 'ISO 9001', 'ISO 14001'],
    autoModules: ['com.aether.ppap', 'com.aether.fmea'],
  },
  pharmaceuticals: {
    slug: 'pharmaceuticals',
    display: 'Pharmaceuticals',
    capabilities: [
      { id: 'gxp-electronic-records', kind: 'compliance', display: 'GxP electronic records', description: '21 CFR Part 11 compliant signatures & audit trails' },
      { id: 'serialization-track-trace', kind: 'tracking', display: 'Serialization & track-and-trace', description: 'Per-unit serial numbers, parent-child aggregation' },
      { id: 'environmental-monitoring', kind: 'quality', display: 'Environmental monitoring', description: 'Cleanroom particle, viable, and pressure differentials' },
    ],
    defaultStandards: ['ISO 13485', 'ISO 9001', 'FDA 21 CFR Part 11', 'GMP', 'BPOM'],
    autoModules: ['com.aether.gxp-records', 'com.aether.batch-genealogy'],
  },
};

const KIND_PILL: Record<Capability['kind'], 'info' | 'success' | 'warning' | 'danger' | 'neutral'> = {
  tracking: 'info',
  quality: 'success',
  maintenance: 'warning',
  safety: 'danger',
  compliance: 'info',
  process: 'neutral',
  sustainability: 'success',
};

export function IndustryProfilePage() {
  const liveIndustry = useIndustry();
  const setIndustry = useSetIndustry();
  const [active, setActive] = useState<string>(liveIndustry ?? 'food-and-beverage');
  const profile = PROFILES[active]!;

  // When the picker changes, push the new profile into the live
  // CapabilityProvider so the rest of the app reshapes itself.
  useEffect(() => {
    if (!profile) return;
    setIndustry(
      profile.slug,
      profile.capabilities.map((c) => c.id),
    );
  }, [profile, setIndustry]);

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>Industry profile</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          Pick the tenant's industry. AETHER-OS auto-activates capabilities, default
          modules, and compliance standards — no per-customer fork. 21 verticals supported.
          The picker below drives the live <code>CapabilityProvider</code>; switch to Manager →
          Inventory to see the columns reshape.
        </p>
      </header>

      <Card>
        <CardBody>
          <Stack direction="row" gap={8} wrap>
            {Object.values(PROFILES).map((p) => (
              <Button
                key={p.slug}
                variant={p.slug === active ? 'primary' : 'secondary'}
                size="sm"
                onClick={() => setActive(p.slug)}
              >
                {p.display}
              </Button>
            ))}
            <Button size="sm" variant="ghost" disabled>
              + 18 more (PR #6)
            </Button>
          </Stack>
        </CardBody>
      </Card>

      <Card>
        <CardHeader
          title={`${profile.display} · ${profile.capabilities.length} auto-capabilities`}
          subtitle="Activated automatically when this profile is set on the tenant."
        />
        <CardBody>
          <Stack gap={8}>
            {profile.capabilities.map((c) => (
              <div
                key={c.id}
                style={{
                  display: 'grid',
                  gridTemplateColumns: '120px 1fr',
                  gap: 12,
                  padding: '8px 0',
                  borderTop: '1px solid var(--aether-border)',
                }}
              >
                <div>
                  <StatusPill kind={KIND_PILL[c.kind]}>{c.kind}</StatusPill>
                </div>
                <div>
                  <div style={{ fontWeight: 600 }}>{c.display}</div>
                  <div style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                    {c.description}
                  </div>
                  <code style={{ fontSize: 11, color: 'var(--aether-fg-muted)' }}>
                    useCapability("{c.id}")
                  </code>
                </div>
              </div>
            ))}
          </Stack>
        </CardBody>
      </Card>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <Card>
          <CardHeader title="Auto-installed modules" subtitle="Provisioned at tenant onboarding" />
          <CardBody>
            {profile.autoModules.length === 0 ? (
              <span style={{ fontSize: 13, color: 'var(--aether-fg-muted)' }}>None</span>
            ) : (
              <Stack gap={4}>
                {profile.autoModules.map((m) => (
                  <code key={m} style={{ fontSize: 13 }}>
                    {m}
                  </code>
                ))}
              </Stack>
            )}
          </CardBody>
        </Card>
        <Card>
          <CardHeader title="Default compliance standards" subtitle="Seeded into aether-compliance" />
          <CardBody>
            <Stack direction="row" gap={6} wrap>
              {profile.defaultStandards.map((s) => (
                <StatusPill key={s} kind="info">
                  {s}
                </StatusPill>
              ))}
            </Stack>
          </CardBody>
        </Card>
      </div>
    </Stack>
  );
}
