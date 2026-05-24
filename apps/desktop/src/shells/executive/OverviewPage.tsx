import { Card, CardBody, CardHeader, Stack } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

const KPIS = [
  { labelKey: 'page.overview.kpi.oee', value: '—', hintKey: 'page.overview.hint.telemetry' },
  { labelKey: 'page.overview.kpi.throughput', value: '—', hintKey: 'page.overview.hint.telemetry' },
  { labelKey: 'page.overview.kpi.quality', value: '—', hintKey: 'page.overview.hint.telemetry' },
  { labelKey: 'page.overview.kpi.margin', value: '—', hintKey: 'page.overview.hint.finance' },
];

export function OverviewPage() {
  const { t } = useTranslation();
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.overview.title')}</h1>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 16,
        }}
      >
        {KPIS.map((k) => (
          <Card key={k.labelKey}>
            <CardHeader title={t(k.labelKey)} />
            <CardBody>
              <div style={{ fontSize: 36, fontWeight: 700 }}>{k.value}</div>
              <div style={{ marginTop: 4, fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                {t(k.hintKey)}
              </div>
            </CardBody>
          </Card>
        ))}
      </div>
    </Stack>
  );
}
