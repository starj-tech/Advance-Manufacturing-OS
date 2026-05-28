import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

export function App() {
  return (
    <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', padding: 24 }}>
      <div style={{ width: '100%', maxWidth: 560 }}>
        <Card>
          <CardHeader
            title="AETHER-OS · Aplikasi Utama"
            subtitle="Platform produksi untuk perusahaan manufaktur"
          />
          <CardBody>
            <Stack gap={12}>
              <p style={{ margin: 0, fontSize: 14, lineHeight: 1.6 }}>
                Tempat perusahaan mendaftar, memilih industri & paket langganan (Standard Node /
                Advanced Automata / Global Enterprise), lalu mengakses shell multi-peran (Executive,
                Manager, Employee, Developer) yang disesuaikan per industri.
              </p>
              <StatusPill kind="info">Sedang dibangun</StatusPill>
            </Stack>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
