import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

interface StandardSummary {
  slug: string;
  display: string;
  jurisdiction: string;
  kind: 'international' | 'industry' | 'regulation' | 'national';
  controlsTotal: number;
  status: 'compliant' | 'needs-review' | 'non-compliant' | 'not-run';
  lastReport: string;
}

const ROWS: StandardSummary[] = [
  { slug: 'iso-9001', display: 'ISO 9001:2015', jurisdiction: 'International', kind: 'international', controlsTotal: 5, status: 'compliant', lastReport: '2026-05-04' },
  { slug: 'iso-22000', display: 'ISO 22000', jurisdiction: 'International', kind: 'international', controlsTotal: 3, status: 'compliant', lastReport: '2026-05-04' },
  { slug: 'iatf-16949', display: 'IATF 16949', jurisdiction: 'International', kind: 'industry', controlsTotal: 4, status: 'needs-review', lastReport: '2026-05-03' },
  { slug: 'gdpr', display: 'GDPR', jurisdiction: 'European Union', kind: 'regulation', controlsTotal: 4, status: 'compliant', lastReport: '2026-05-04' },
  { slug: 'fda-21-cfr-11', display: 'FDA 21 CFR Part 11', jurisdiction: 'United States', kind: 'regulation', controlsTotal: 3, status: 'non-compliant', lastReport: '2026-05-04' },
  { slug: 'bpom', display: 'BPOM', jurisdiction: 'Indonesia', kind: 'national', controlsTotal: 3, status: 'compliant', lastReport: '2026-05-04' },
  { slug: 'sni', display: 'SNI', jurisdiction: 'Indonesia', kind: 'national', controlsTotal: 2, status: 'not-run', lastReport: '—' },
  { slug: 'reach', display: 'REACH', jurisdiction: 'European Union', kind: 'regulation', controlsTotal: 2, status: 'compliant', lastReport: '2026-04-29' },
];

const PILL: Record<StandardSummary['status'], 'success' | 'warning' | 'danger' | 'neutral'> = {
  compliant: 'success',
  'needs-review': 'warning',
  'non-compliant': 'danger',
  'not-run': 'neutral',
};

const KIND_PILL: Record<StandardSummary['kind'], 'info' | 'success' | 'warning' | 'neutral'> = {
  international: 'info',
  industry: 'success',
  regulation: 'warning',
  national: 'neutral',
};

export function CompliancePage() {
  const rollup = {
    total: ROWS.length,
    compliant: ROWS.filter((r) => r.status === 'compliant').length,
    needsReview: ROWS.filter((r) => r.status === 'needs-review').length,
    nonCompliant: ROWS.filter((r) => r.status === 'non-compliant').length,
  };

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>Compliance attestation</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          Live status across every regulatory + voluntary standard you've enrolled in. Each
          report is signed by the AETHER attestation service so external auditors can verify
          it without trusting us.
        </p>
      </header>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16 }}>
        {[
          { label: 'Enrolled', value: rollup.total },
          { label: 'Compliant', value: rollup.compliant, kind: 'success' as const },
          { label: 'Needs review', value: rollup.needsReview, kind: 'warning' as const },
          { label: 'Non-compliant', value: rollup.nonCompliant, kind: 'danger' as const },
        ].map((m) => (
          <Card key={m.label}>
            <CardBody>
              <div style={{ fontSize: 12, color: 'var(--aether-fg-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                {m.label}
              </div>
              <div style={{ fontSize: 36, fontWeight: 700, marginTop: 4 }}>{m.value}</div>
            </CardBody>
          </Card>
        ))}
      </div>

      <Card padded={false}>
        <CardHeader
          title="Standards"
          subtitle="Demo data — real probes wire up in PR #6 alongside the attestation signing key."
        />
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                <th style={th}>Standard</th>
                <th style={th}>Jurisdiction</th>
                <th style={th}>Kind</th>
                <th style={th}>Controls</th>
                <th style={th}>Status</th>
                <th style={th}>Last report</th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map((r) => (
                <tr key={r.slug} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <div style={{ fontWeight: 600 }}>{r.display}</div>
                    <code style={{ fontSize: 11, color: 'var(--aether-fg-muted)' }}>{r.slug}</code>
                  </td>
                  <td style={td}>{r.jurisdiction}</td>
                  <td style={td}>
                    <StatusPill kind={KIND_PILL[r.kind]}>{r.kind}</StatusPill>
                  </td>
                  <td style={td}>{r.controlsTotal}</td>
                  <td style={td}>
                    <StatusPill kind={PILL[r.status]}>{r.status}</StatusPill>
                  </td>
                  <td style={td}>{r.lastReport}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}

const th: React.CSSProperties = {
  padding: '10px 16px',
  fontSize: 12,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
};
const td: React.CSSProperties = { padding: '10px 16px' };
