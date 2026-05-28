import { useState } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

type Signal = 'normal' | 'elevated' | 'critical';

interface MachineHealth {
  code: string;
  name: string;
  health: number; // 0-100
  rulDays: number;
  vibration: Signal;
  thermal: Signal;
}

// Demo telemetry; PR #7 supplies RUL via the ONNX model, PR #3 the live
// vibration/thermal tags.
const MACHINES: MachineHealth[] = [
  {
    code: 'WELD-04',
    name: 'Robotic welder W4',
    health: 41,
    rulDays: 5,
    vibration: 'critical',
    thermal: 'elevated',
  },
  {
    code: 'PRESS-01',
    name: 'Hydraulic press 250t',
    health: 62,
    rulDays: 11,
    vibration: 'elevated',
    thermal: 'normal',
  },
  {
    code: 'PAINT-02',
    name: 'Powder coat line',
    health: 76,
    rulDays: 32,
    vibration: 'normal',
    thermal: 'elevated',
  },
  {
    code: 'CONV-07',
    name: 'Conveyor line 7',
    health: 80,
    rulDays: 40,
    vibration: 'elevated',
    thermal: 'normal',
  },
  {
    code: 'CNC-12',
    name: 'CNC mill A12',
    health: 88,
    rulDays: 73,
    vibration: 'normal',
    thermal: 'normal',
  },
];

const SIGNAL_PILL: Record<Signal, 'success' | 'warning' | 'danger'> = {
  normal: 'success',
  elevated: 'warning',
  critical: 'danger',
};

function healthKind(h: number): 'success' | 'warning' | 'danger' {
  if (h >= 75) return 'success';
  if (h >= 50) return 'warning';
  return 'danger';
}

function recommendation(rulDays: number): {
  key: string;
  kind: 'success' | 'warning' | 'danger';
} {
  if (rulDays < 14) return { key: 'page.maintenance.rec.scheduleNow', kind: 'danger' };
  if (rulDays < 45) return { key: 'page.maintenance.rec.planSoon', kind: 'warning' };
  return { key: 'page.maintenance.rec.monitor', kind: 'success' };
}

const BAR_COLOR: Record<'success' | 'warning' | 'danger', string> = {
  success: 'var(--aether-success)',
  warning: 'var(--aether-warning)',
  danger: 'var(--aether-danger)',
};

export function MaintenancePage() {
  const { t } = useTranslation();
  const [scheduled, setScheduled] = useState<Set<string>>(new Set());

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.maintenance.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.maintenance.desc')}
        </p>
      </header>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
          gap: 16,
        }}
      >
        {MACHINES.map((m) => {
          const hKind = healthKind(m.health);
          const rec = recommendation(m.rulDays);
          const isScheduled = scheduled.has(m.code);
          return (
            <Card key={m.code}>
              <CardHeader title={m.name} subtitle={m.code}>
                <div style={{ position: 'absolute', top: 16, right: 16 }}>
                  <StatusPill kind={rec.kind}>{t(rec.key)}</StatusPill>
                </div>
              </CardHeader>
              <CardBody>
                <Stack gap={10}>
                  <div>
                    <Stack direction="row" justify="space-between">
                      <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                        {t('page.maintenance.health')}
                      </span>
                      <span style={{ fontSize: 12, fontWeight: 600 }}>{m.health}%</span>
                    </Stack>
                    <div
                      style={{
                        marginTop: 4,
                        height: 8,
                        borderRadius: 999,
                        background: 'var(--aether-border)',
                        overflow: 'hidden',
                      }}
                    >
                      <div
                        style={{
                          width: `${m.health}%`,
                          height: '100%',
                          background: BAR_COLOR[hKind],
                        }}
                      />
                    </div>
                  </div>

                  <Stack direction="row" justify="space-between" align="center">
                    <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                      {t('page.maintenance.rul')}
                    </span>
                    <StatusPill kind={recommendation(m.rulDays).kind}>
                      {t('page.maintenance.rulDays', { days: m.rulDays })}
                    </StatusPill>
                  </Stack>

                  <Stack direction="row" gap={8} wrap>
                    <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                      {t('page.maintenance.vibration')}
                    </span>
                    <StatusPill kind={SIGNAL_PILL[m.vibration]}>
                      {t(`page.maintenance.signal.${m.vibration}`)}
                    </StatusPill>
                    <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                      {t('page.maintenance.thermal')}
                    </span>
                    <StatusPill kind={SIGNAL_PILL[m.thermal]}>
                      {t(`page.maintenance.signal.${m.thermal}`)}
                    </StatusPill>
                  </Stack>

                  {isScheduled ? (
                    <StatusPill kind="info">{t('page.maintenance.scheduled')}</StatusPill>
                  ) : (
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={() => setScheduled((s) => new Set(s).add(m.code))}
                    >
                      {t('page.maintenance.createWO')}
                    </Button>
                  )}
                </Stack>
              </CardBody>
            </Card>
          );
        })}
      </div>

      <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
        {t('page.maintenance.note')}
      </span>
    </Stack>
  );
}
