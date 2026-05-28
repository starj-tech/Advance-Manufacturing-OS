import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

export function App() {
  return (
    <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', padding: 24 }}>
      <div style={{ width: '100%', maxWidth: 560 }}>
        <Card>
          <CardHeader title="AETHER-OS · Aplikasi IT" subtitle="Untuk tim IT perusahaan client" />
          <CardBody>
            <Stack gap={12}>
              <p style={{ margin: 0, fontSize: 14, lineHeight: 1.6 }}>
                Pengelolaan akun & peran (impor CSV, reset kata sandi), binding perangkat/protokol
                (OPC-UA/MQTT), pemasangan & kill-switch modul, serta audit keamanan untuk armada
                pabrik.
              </p>
              <StatusPill kind="info">Sedang dibangun</StatusPill>
            </Stack>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
