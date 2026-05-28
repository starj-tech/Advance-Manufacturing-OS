import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';
import type { MachineStatus } from '@aether/rpc-contracts';
import { useMachines } from '../../data/useMachines';

const STATUS_KIND: Record<MachineStatus, StatusKind> = {
  running: 'success',
  idle: 'neutral',
  paused: 'warning',
  fault: 'danger',
  maintenance: 'warning',
  offline: 'neutral',
};

const mutedStyle = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };

export function MachinesPage() {
  const { t } = useTranslation();
  const { data: machines = [], isLoading, isError } = useMachines();

  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.machines.title')}</h1>

      {isLoading ? <p style={mutedStyle}>{t('common.loading')}</p> : null}
      {isError ? <p style={mutedStyle}>{t('common.error')}</p> : null}
      {!isLoading && !isError && machines.length === 0 ? (
        <p style={mutedStyle}>{t('common.empty')}</p>
      ) : null}

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
          gap: 16,
        }}
      >
        {machines.map((m) => (
          <Card key={m.id}>
            <CardHeader title={m.name} subtitle={m.code} />
            <CardBody>
              <Stack direction="row" align="center" justify="space-between">
                <StatusPill kind={STATUS_KIND[m.status]}>
                  {t(`page.machines.status.${m.status}`)}
                </StatusPill>
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
