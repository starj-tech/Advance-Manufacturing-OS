import { Card, CardBody, CardHeader, Stack, StatusPill, Button } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

interface Recommendation {
  ticker: string;
  name: string;
  spot: string;
  sma: string;
  action: 'buy-now' | 'wait' | 'hold';
  savings?: string;
  rationale: string;
}

const DEMO: Recommendation[] = [
  {
    ticker: 'HRC',
    name: 'Hot-rolled coil steel',
    spot: '$612 / MT',
    sma: '$728 / MT',
    action: 'buy-now',
    savings: '15.9%',
    rationale: 'HRC spot is 15.9% below 90-day SMA; lock 6 months of supply.',
  },
  {
    ticker: 'ALI',
    name: 'Aluminum',
    spot: '$2,450 / MT',
    sma: '$2,420 / MT',
    action: 'hold',
    rationale: 'Spot within ±10% of average. Stay the course.',
  },
  {
    ticker: 'BZ',
    name: 'Brent crude',
    spot: '$92.4 / bbl',
    sma: '$78.1 / bbl',
    action: 'wait',
    rationale: 'Brent up 18% on supply news; defer fuel & polymer purchases.',
  },
  {
    ticker: 'PP',
    name: 'Polypropylene',
    spot: '$1,180 / MT',
    sma: '$1,310 / MT',
    action: 'buy-now',
    savings: '9.9%',
    rationale: 'PP near 12-month low; 3-month buy recommended.',
  },
];

const PILL: Record<Recommendation['action'], 'success' | 'warning' | 'neutral'> = {
  'buy-now': 'success',
  wait: 'warning',
  hold: 'neutral',
};

export function SupplyChainPage() {
  const { t } = useTranslation();
  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.supply.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.supply.intro')}
        </p>
      </header>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
          gap: 16,
        }}
      >
        {DEMO.map((r) => (
          <Card key={r.ticker}>
            <CardHeader title={r.name} subtitle={`${r.ticker} · spot ${r.spot}`}>
              <div style={{ position: 'absolute', top: 16, right: 16 }}>
                <StatusPill kind={PILL[r.action]}>
                  {t(`page.supply.action.${r.action}`)}
                  {r.savings ? ` ${t('page.supply.save', { savings: r.savings })}` : ''}
                </StatusPill>
              </div>
            </CardHeader>
            <CardBody>
              <p style={{ margin: 0, fontSize: 13 }}>{r.rationale}</p>
              <p style={{ margin: '8px 0 12px', fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                {t('page.supply.sma', { sma: r.sma })}
              </p>
              <Stack direction="row" gap={8}>
                <Button size="sm" variant="primary" disabled>
                  {t('page.supply.accept')}
                </Button>
                <Button size="sm" variant="ghost" disabled>
                  {t('page.supply.dismiss')}
                </Button>
              </Stack>
            </CardBody>
          </Card>
        ))}
      </div>
    </Stack>
  );
}
