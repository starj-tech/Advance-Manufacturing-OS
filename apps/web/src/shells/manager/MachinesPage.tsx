import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

const MACHINES = [
  { code: 'PRESS-01', name: 'Hydraulic press 250t', status: 'running', kind: 'success' as const },
  { code: 'CNC-12', name: 'CNC mill A12', status: 'idle', kind: 'neutral' as const },
  { code: 'WELD-04', name: 'Robotic welder W4', status: 'fault', kind: 'danger' as const },
  { code: 'PAINT-02', name: 'Powder coat line', status: 'maintenance', kind: 'warning' as const },
];

export function MachinesPage() {
  const { t } = useTranslation();
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.machines.title')}</h1>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
          gap: 16,
        }}
      >
        {MACHINES.map((m) => (
          <Card key={m.code}>
            <CardHeader title={m.name} subtitle={m.code} />
            <CardBody>
              <Stack direction="row" align="center" justify="space-between">
                <StatusPill kind={m.kind}>{t(`page.machines.status.${m.status}`)}</StatusPill>
                <span style={{ color: 'var(--aether-fg-muted)', fontSize: 12 }}>—</span>
              </Stack>
              <p style={{ margin: '12px 0 0', color: 'var(--aether-fg-muted)', fontSize: 12 }}>
                {t('page.machines.note')}
              </p>
            </CardBody>
          </Card>
        ))}
      </div>
    </Stack>
  );
}
