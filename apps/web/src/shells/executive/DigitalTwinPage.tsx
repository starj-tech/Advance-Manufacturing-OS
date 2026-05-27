import { useState } from 'react';
import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

type Status = 'running' | 'idle' | 'fault' | 'maintenance';

interface PlantMachine {
  code: string;
  name: string;
  status: Status;
  throughput: string;
  temperature: string;
  cycle: string;
}

// Demo floor; PR #6 replaces this 2D grid with a React Three Fiber scene
// and overlays live telemetry on the model.
const MACHINES: PlantMachine[] = [
  {
    code: 'PRESS-01',
    name: 'Hydraulic press 250t',
    status: 'running',
    throughput: '41/h',
    temperature: '64°C',
    cycle: '42s',
  },
  {
    code: 'CNC-12',
    name: 'CNC mill A12',
    status: 'idle',
    throughput: '0/h',
    temperature: '31°C',
    cycle: '—',
  },
  {
    code: 'WELD-04',
    name: 'Robotic welder W4',
    status: 'fault',
    throughput: '0/h',
    temperature: '95°C',
    cycle: '—',
  },
  {
    code: 'PAINT-02',
    name: 'Powder coat line',
    status: 'maintenance',
    throughput: '0/h',
    temperature: '40°C',
    cycle: '—',
  },
  {
    code: 'CONV-07',
    name: 'Conveyor line 7',
    status: 'running',
    throughput: '120/h',
    temperature: '28°C',
    cycle: '8s',
  },
  {
    code: 'ASSY-03',
    name: 'Assembly cell 3',
    status: 'running',
    throughput: '36/h',
    temperature: '24°C',
    cycle: '55s',
  },
];

const STATUS_PILL: Record<Status, 'success' | 'neutral' | 'danger' | 'warning'> = {
  running: 'success',
  idle: 'neutral',
  fault: 'danger',
  maintenance: 'warning',
};

const STATUS_COLOR: Record<Status, string> = {
  running: 'var(--aether-success)',
  idle: 'var(--aether-fg-muted)',
  fault: 'var(--aether-danger)',
  maintenance: 'var(--aether-warning)',
};

export function DigitalTwinPage() {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<string | null>(null);
  const active = MACHINES.find((m) => m.code === selected) ?? null;

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.digitalTwin.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.digitalTwin.subtitle')}
        </p>
      </header>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 2fr) minmax(220px, 1fr)',
          gap: 16,
        }}
      >
        <Card>
          <CardBody>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))',
                gap: 12,
              }}
            >
              {MACHINES.map((m) => {
                const isActive = m.code === selected;
                return (
                  <button
                    key={m.code}
                    onClick={() => setSelected(m.code)}
                    style={{
                      textAlign: 'left',
                      padding: 12,
                      borderRadius: 8,
                      border: `1px solid ${isActive ? 'var(--aether-accent)' : 'var(--aether-border)'}`,
                      background: isActive ? 'var(--aether-bg)' : 'transparent',
                      color: 'var(--aether-fg)',
                      cursor: 'pointer',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 8,
                    }}
                  >
                    <Stack direction="row" align="center" gap={8}>
                      <span
                        aria-hidden
                        style={{
                          width: 10,
                          height: 10,
                          borderRadius: '50%',
                          background: STATUS_COLOR[m.status],
                        }}
                      />
                      <code style={{ fontSize: 12, fontWeight: 600 }}>{m.code}</code>
                    </Stack>
                    <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>{m.name}</span>
                  </button>
                );
              })}
            </div>
          </CardBody>
        </Card>

        <Card>
          <CardHeader
            title={active ? active.name : t('page.digitalTwin.title')}
            subtitle={active?.code}
          />
          <CardBody>
            {active ? (
              <Stack gap={12}>
                <StatusPill kind={STATUS_PILL[active.status]}>
                  {t(`page.machines.status.${active.status}`)}
                </StatusPill>
                <Reading
                  label={t('page.digitalTwin.reading.throughput')}
                  value={active.throughput}
                />
                <Reading
                  label={t('page.digitalTwin.reading.temperature')}
                  value={active.temperature}
                />
                <Reading label={t('page.digitalTwin.reading.cycle')} value={active.cycle} />
              </Stack>
            ) : (
              <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
                {t('page.digitalTwin.selectHint')}
              </p>
            )}
          </CardBody>
        </Card>
      </div>

      <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
        {t('page.digitalTwin.note')}
      </span>
    </Stack>
  );
}

function Reading({ label, value }: { label: string; value: string }) {
  return (
    <Stack direction="row" justify="space-between" align="center">
      <span style={{ fontSize: 13, color: 'var(--aether-fg-muted)' }}>{label}</span>
      <span style={{ fontSize: 16, fontWeight: 700 }}>{value}</span>
    </Stack>
  );
}
