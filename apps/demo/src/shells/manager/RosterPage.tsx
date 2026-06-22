import { useState } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

type Presence = 'onsite' | 'offsite' | 'break';
type Shift = 'day' | 'night' | 'swing';

interface Operator {
  id: string;
  name: string;
  station: string;
  shift: Shift;
  shiftTime: string;
  presence: Presence;
  certsValid: number;
  certsTotal: number;
  onShift: boolean;
}

// Demo roster; PR #5 supplies live presence over mDNS, the HR module
// supplies shift assignments.
const OPERATORS: Operator[] = [
  {
    id: 'op-001',
    name: 'Sari Wijaya',
    station: 'Welding bay',
    shift: 'day',
    shiftTime: '06:00–14:00',
    presence: 'onsite',
    certsValid: 3,
    certsTotal: 4,
    onShift: true,
  },
  {
    id: 'op-002',
    name: 'Budi Santosa',
    station: 'CNC cell',
    shift: 'day',
    shiftTime: '06:00–14:00',
    presence: 'onsite',
    certsValid: 4,
    certsTotal: 4,
    onShift: true,
  },
  {
    id: 'op-003',
    name: 'Dewi Lestari',
    station: 'Paint line',
    shift: 'swing',
    shiftTime: '14:00–22:00',
    presence: 'break',
    certsValid: 2,
    certsTotal: 3,
    onShift: true,
  },
  {
    id: 'op-004',
    name: 'Andi Pratama',
    station: 'Press shop',
    shift: 'night',
    shiftTime: '22:00–06:00',
    presence: 'offsite',
    certsValid: 4,
    certsTotal: 4,
    onShift: false,
  },
  {
    id: 'op-005',
    name: 'Rina Hapsari',
    station: 'QC lab',
    shift: 'day',
    shiftTime: '06:00–14:00',
    presence: 'onsite',
    certsValid: 5,
    certsTotal: 5,
    onShift: true,
  },
];

const PRESENCE_PILL: Record<Presence, 'success' | 'warning' | 'neutral'> = {
  onsite: 'success',
  break: 'warning',
  offsite: 'neutral',
};

export function RosterPage() {
  const { t } = useTranslation();
  const [onShiftOnly, setOnShiftOnly] = useState(false);

  const rows = OPERATORS.filter((o) => !onShiftOnly || o.onShift);

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.roster.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.roster.desc')}
        </p>
      </header>

      <Card padded={false}>
        <CardHeader>
          <Button
            size="sm"
            variant={onShiftOnly ? 'primary' : 'ghost'}
            onClick={() => setOnShiftOnly((v) => !v)}
          >
            {t('page.roster.onShiftOnly')}
          </Button>
        </CardHeader>
        <CardBody>
          {rows.length === 0 ? (
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              {t('page.roster.empty')}
            </p>
          ) : (
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                  <th style={th}>{t('page.roster.col.operator')}</th>
                  <th style={th}>{t('page.roster.col.station')}</th>
                  <th style={th}>{t('page.roster.col.shift')}</th>
                  <th style={th}>{t('page.roster.col.presence')}</th>
                  <th style={th}>{t('page.roster.col.certs')}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((o) => {
                  const certsOk = o.certsValid === o.certsTotal;
                  return (
                    <tr key={o.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                      <td style={td}>{o.name}</td>
                      <td style={td}>{o.station}</td>
                      <td style={td}>
                        {t(`page.roster.shift.${o.shift}`)} · {o.shiftTime}
                      </td>
                      <td style={td}>
                        <StatusPill kind={PRESENCE_PILL[o.presence]}>
                          {t(`page.roster.presence.${o.presence}`)}
                        </StatusPill>
                      </td>
                      <td style={td}>
                        <StatusPill kind={certsOk ? 'success' : 'warning'}>
                          {t('page.roster.certsValid', {
                            valid: o.certsValid,
                            total: o.certsTotal,
                          })}
                        </StatusPill>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </CardBody>
      </Card>

      <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>{t('page.roster.note')}</span>
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
