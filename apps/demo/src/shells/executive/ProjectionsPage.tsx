import { useMemo, useState } from 'react';
import { Button, Card, CardBody, Stack } from '@aether/ui-kit';
import { formatMoney, formatNumber, useTranslation } from '@aether/i18n';

type Horizon = 30 | 60 | 90;
type Scenario = 'base' | 'optimistic' | 'pessimistic';

interface Product {
  name: string;
  monthlyVolume: number;
  unitMargin: number; // major currency units
}

// Demo book; PR #7 replaces the deterministic model with an LSTM forecast
// + Monte Carlo simulation and FX-normalizes against commodity_prices.
const PRODUCTS: Product[] = [
  { name: 'Stator core 3kW', monthlyVolume: 1500, unitMargin: 18 },
  { name: 'Rotor shaft 12mm', monthlyVolume: 2200, unitMargin: 7 },
  { name: 'Front brake pads', monthlyVolume: 900, unitMargin: 12 },
  { name: 'Powder coat service', monthlyVolume: 600, unitMargin: 25 },
];

const HORIZONS: Horizon[] = [30, 60, 90];
const SCENARIOS: Scenario[] = ['base', 'optimistic', 'pessimistic'];
const VOLUME_MULT: Record<Scenario, number> = { base: 1, optimistic: 1.15, pessimistic: 0.85 };
const EXPOSURE_PCT: Record<Scenario, number> = { base: 22, optimistic: 18, pessimistic: 31 };

export function ProjectionsPage() {
  const { t, locale } = useTranslation();
  const [horizon, setHorizon] = useState<Horizon>(30);
  const [scenario, setScenario] = useState<Scenario>('base');

  const money = (amount: number) =>
    formatMoney(locale.tag, { amount, currency: locale.defaultCurrency });

  const rows = useMemo(() => {
    const months = horizon / 30;
    const mult = VOLUME_MULT[scenario];
    return PRODUCTS.map((p) => {
      const volume = Math.round(p.monthlyVolume * months * mult);
      const pnl = volume * p.unitMargin;
      return { ...p, volume, pnl };
    });
  }, [horizon, scenario]);

  const totalPnl = rows.reduce((s, r) => s + r.pnl, 0);
  const totalVolume = rows.reduce((s, r) => s + r.volume, 0);
  const maxPnl = Math.max(...rows.map((r) => r.pnl), 1);

  const kpis = [
    { label: t('page.projections.kpi.margin'), value: money(totalPnl) },
    { label: t('page.projections.kpi.volume'), value: formatNumber(locale.tag, totalVolume, 0) },
    { label: t('page.projections.kpi.exposure'), value: `${EXPOSURE_PCT[scenario]}%` },
  ];

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.projections.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.projections.desc')}
        </p>
      </header>

      <Stack direction="row" gap={24} wrap>
        <Stack gap={4}>
          <span
            style={{ fontSize: 11, textTransform: 'uppercase', color: 'var(--aether-fg-muted)' }}
          >
            {t('page.projections.horizon')}
          </span>
          <Stack direction="row" gap={4}>
            {HORIZONS.map((h) => (
              <Button
                key={h}
                size="sm"
                variant={horizon === h ? 'primary' : 'ghost'}
                onClick={() => setHorizon(h)}
              >
                {t('page.projections.dayShort', { days: h })}
              </Button>
            ))}
          </Stack>
        </Stack>
        <Stack gap={4}>
          <span
            style={{ fontSize: 11, textTransform: 'uppercase', color: 'var(--aether-fg-muted)' }}
          >
            {t('page.projections.scenario')}
          </span>
          <Stack direction="row" gap={4}>
            {SCENARIOS.map((s) => (
              <Button
                key={s}
                size="sm"
                variant={scenario === s ? 'primary' : 'ghost'}
                onClick={() => setScenario(s)}
              >
                {t(`page.projections.scenario.${s}`)}
              </Button>
            ))}
          </Stack>
        </Stack>
      </Stack>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
          gap: 16,
        }}
      >
        {kpis.map((k) => (
          <Card key={k.label}>
            <CardBody>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--aether-fg-muted)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.05em',
                }}
              >
                {k.label}
              </div>
              <div style={{ fontSize: 30, fontWeight: 700, marginTop: 4 }}>{k.value}</div>
            </CardBody>
          </Card>
        ))}
      </div>

      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                <th style={th}>{t('page.projections.col.product')}</th>
                <th style={th}>{t('page.projections.col.volume')}</th>
                <th style={th}>{t('page.projections.col.unitMargin')}</th>
                <th style={th}>{t('page.projections.col.pnl')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.name} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{r.name}</td>
                  <td style={td}>{formatNumber(locale.tag, r.volume, 0)}</td>
                  <td style={td}>{money(r.unitMargin)}</td>
                  <td style={td}>
                    <div style={{ fontWeight: 600 }}>{money(r.pnl)}</div>
                    <div
                      style={{
                        marginTop: 4,
                        height: 6,
                        width: `${(r.pnl / maxPnl) * 100}%`,
                        minWidth: 4,
                        borderRadius: 999,
                        background: 'var(--aether-accent)',
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>

      <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
        {t('page.projections.note')}
      </span>
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
