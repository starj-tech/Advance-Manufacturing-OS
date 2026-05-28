import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

export function App() {
  return (
    <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', padding: 24 }}>
      <div style={{ width: '100%', maxWidth: 560 }}>
        <Card>
          <CardHeader
            title="AETHER-OS · Konsol Developer"
            subtitle="Panel internal vendor (pemilik perangkat lunak)"
          />
          <CardBody>
            <Stack gap={12}>
              <p style={{ margin: 0, fontSize: 14, lineHeight: 1.6 }}>
                Kontrol lintas-tenant: siklus hidup perusahaan, langganan Stripe, penerbitan &
                penandatanganan modul, feature flag / kill-switch, dan kesehatan platform.
              </p>
              <StatusPill kind="info">Sedang dibangun</StatusPill>
            </Stack>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
