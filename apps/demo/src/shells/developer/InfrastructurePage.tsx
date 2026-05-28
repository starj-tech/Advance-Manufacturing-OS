import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

const SERVICES = [
  { name: 'SQLite (local)', kind: 'success', detail: 'WAL mode · 0 ms p99' },
  { name: 'Supabase Realtime', kind: 'warning', detail: 'reconnecting…' },
  { name: 'OPC-UA bridge', kind: 'neutral', detail: 'no endpoint configured' },
  { name: 'MQTT bridge', kind: 'neutral', detail: 'no endpoint configured' },
  { name: 'mDNS SOS broadcaster', kind: 'success', detail: 'listening on en0' },
] as const;

export function InfrastructurePage() {
  const { t } = useTranslation();
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.infra.title')}</h1>
      <Stack gap={8}>
        {SERVICES.map((s) => (
          <Card key={s.name}>
            <CardHeader title={s.name} subtitle={s.detail}>
              <div style={{ position: 'absolute', top: 16, right: 16 }}>
                <StatusPill kind={s.kind}>{s.kind}</StatusPill>
              </div>
            </CardHeader>
            <CardBody>
              <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
                {t('page.infra.note')}
              </p>
            </CardBody>
          </Card>
        ))}
      </Stack>
    </Stack>
  );
}
