import { Card, CardBody, CardHeader, Stack, StatusPill, Button } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';
import type { WorkOrderStatus } from '@aether/rpc-contracts';
import { useWorkOrders } from '../../data/useWorkOrders';

const STATUS_KIND: Record<WorkOrderStatus, StatusKind> = {
  draft: 'neutral',
  released: 'info',
  running: 'success',
  paused: 'warning',
  completed: 'success',
  canceled: 'danger',
};

const mutedStyle = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };

export function WorkOrdersPage() {
  const { t } = useTranslation();
  const { data: orders = [], isLoading, isError } = useWorkOrders();

  return (
    <Stack gap={16}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.wo.title')}</h1>
        <Button variant="primary" size="md" disabled>
          {t('page.wo.new')}
        </Button>
      </header>

      {isLoading ? <p style={mutedStyle}>{t('common.loading')}</p> : null}
      {isError ? <p style={mutedStyle}>{t('common.error')}</p> : null}

      <Card padded={false}>
        <CardHeader title={t('page.wo.queueTitle')} subtitle={t('page.wo.queueSubtitle')} />
        <CardBody>
          {!isLoading && !isError && orders.length === 0 ? (
            <p style={mutedStyle}>{t('common.empty')}</p>
          ) : (
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                  <th style={th}>{t('page.wo.col.code')}</th>
                  <th style={th}>{t('page.wo.col.quantity')}</th>
                  <th style={th}>{t('page.wo.col.status')}</th>
                  <th style={th}></th>
                </tr>
              </thead>
              <tbody>
                {orders.map((o) => (
                  <tr key={o.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                    <td style={td}>{o.code}</td>
                    <td style={td}>
                      {o.qtyDone} / {o.qtyPlanned}
                    </td>
                    <td style={td}>
                      <StatusPill kind={STATUS_KIND[o.status]}>
                        {t(`page.wo.status.${o.status}`)}
                      </StatusPill>
                    </td>
                    <td style={{ ...td, textAlign: 'right' }}>
                      <Button size="sm" variant="ghost" disabled>
                        {t('page.wo.open')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardBody>
      </Card>
    </Stack>
  );
}

const th: React.CSSProperties = {
  padding: '10px 16px',
  fontSize: 12,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
};
const td: React.CSSProperties = { padding: '10px 16px' };
