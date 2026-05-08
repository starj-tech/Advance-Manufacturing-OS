import { Card, CardBody, CardHeader, Stack } from '@aether/ui-kit';

export function DigitalTwinPage() {
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>Digital twin</h1>
      <Card>
        <CardHeader
          title="Plant viewer"
          subtitle="React Three Fiber scene mounts here in PR #6"
        />
        <CardBody>
          <div
            style={{
              height: 480,
              borderRadius: 8,
              border: '1px dashed var(--aether-border)',
              display: 'grid',
              placeItems: 'center',
              color: 'var(--aether-fg-muted)',
              fontSize: 13,
            }}
          >
            Placeholder · 3D model + live telemetry overlay coming in PR #6
          </div>
        </CardBody>
      </Card>
    </Stack>
  );
}
