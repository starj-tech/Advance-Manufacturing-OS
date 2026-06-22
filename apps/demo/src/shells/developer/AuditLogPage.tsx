import { useMemo, useState } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

type Result = 'allowed' | 'denied';

interface AuditEvent {
  id: string;
  at: string;
  actor: string;
  action: string;
  target: string;
  result: Result;
}

// Demo ledger; PR #2 streams real rows from audit_log via Supabase Realtime.
const EVENTS: AuditEvent[] = [
  {
    id: '01J0A7',
    at: '2026-05-08 06:41:12',
    actor: 'Sari Wijaya',
    action: 'interlock.override',
    target: 'PRESS-01',
    result: 'denied',
  },
  {
    id: '01J0A6',
    at: '2026-05-08 06:12:55',
    actor: 'system',
    action: 'module.install',
    target: 'com.aether.cold-chain',
    result: 'allowed',
  },
  {
    id: '01J0A5',
    at: '2026-05-08 05:58:03',
    actor: 'Budi Santosa',
    action: 'inventory.adjust',
    target: 'MILK-3L (-12 btl)',
    result: 'allowed',
  },
  {
    id: '01J0A4',
    at: '2026-05-08 05:31:47',
    actor: 'Dewi Lestari',
    action: 'cert.grant',
    target: 'op-002 · Welder',
    result: 'allowed',
  },
  {
    id: '01J0A3',
    at: '2026-05-08 04:22:09',
    actor: 'Budi Santosa',
    action: 'machine.command',
    target: 'CNC-12 · start',
    result: 'denied',
  },
  {
    id: '01J0A2',
    at: '2026-05-08 03:09:18',
    actor: 'Sari Wijaya',
    action: 'auth.passkey_register',
    target: 'device:ipad-9',
    result: 'allowed',
  },
  {
    id: '01J0A1',
    at: '2026-05-07 22:48:31',
    actor: 'system',
    action: 'compliance.sign',
    target: 'iso-9001 report',
    result: 'allowed',
  },
  {
    id: '01J0A0',
    at: '2026-05-07 21:14:05',
    actor: 'Andi Pratama',
    action: 'sos.trigger',
    target: 'zone:weld-bay',
    result: 'allowed',
  },
  {
    id: '01J09Z',
    at: '2026-05-07 18:02:44',
    actor: 'Dewi Lestari',
    action: 'config.change',
    target: 'tenant.locale=nl-NL',
    result: 'allowed',
  },
  {
    id: '01J09Y',
    at: '2026-05-07 16:37:20',
    actor: 'Andi Pratama',
    action: 'interlock.override',
    target: 'WELD-04',
    result: 'denied',
  },
];

const RESULT_PILL: Record<Result, 'success' | 'danger'> = {
  allowed: 'success',
  denied: 'danger',
};

const FILTERS: Array<{ value: 'all' | Result; labelKey: string }> = [
  { value: 'all', labelKey: 'page.audit.filterAll' },
  { value: 'allowed', labelKey: 'page.audit.filterAllowed' },
  { value: 'denied', labelKey: 'page.audit.filterDenied' },
];

export function AuditLogPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<'all' | Result>('all');

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return EVENTS.filter((e) => {
      if (filter !== 'all' && e.result !== filter) return false;
      if (!q) return true;
      return (
        e.actor.toLowerCase().includes(q) ||
        e.action.toLowerCase().includes(q) ||
        e.target.toLowerCase().includes(q)
      );
    });
  }, [query, filter]);

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.audit.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.audit.desc')}
        </p>
      </header>

      <Card padded={false}>
        <CardHeader>
          <Stack direction="row" align="center" justify="space-between" wrap gap={12}>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t('page.audit.searchPlaceholder')}
              style={{
                flex: 1,
                minWidth: 220,
                background: 'var(--aether-bg)',
                color: 'var(--aether-fg)',
                border: '1px solid var(--aether-border)',
                borderRadius: 6,
                padding: '7px 10px',
                fontSize: 13,
              }}
            />
            <Stack direction="row" gap={4}>
              {FILTERS.map((f) => (
                <Button
                  key={f.value}
                  size="sm"
                  variant={filter === f.value ? 'primary' : 'ghost'}
                  onClick={() => setFilter(f.value)}
                >
                  {t(f.labelKey)}
                </Button>
              ))}
            </Stack>
          </Stack>
        </CardHeader>
        <CardBody>
          {rows.length === 0 ? (
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              {t('page.audit.empty')}
            </p>
          ) : (
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                  <th style={th}>{t('page.audit.col.time')}</th>
                  <th style={th}>{t('page.audit.col.actor')}</th>
                  <th style={th}>{t('page.audit.col.action')}</th>
                  <th style={th}>{t('page.audit.col.target')}</th>
                  <th style={th}>{t('page.audit.col.result')}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((e) => (
                  <tr key={e.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                    <td style={td}>{e.at}</td>
                    <td style={td}>{e.actor}</td>
                    <td style={td}>
                      <code style={{ fontSize: 12 }}>{e.action}</code>
                    </td>
                    <td style={td}>{e.target}</td>
                    <td style={td}>
                      <StatusPill kind={RESULT_PILL[e.result]}>
                        {t(`page.audit.result.${e.result}`)}
                      </StatusPill>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardBody>
      </Card>

      <Stack direction="row" align="center" justify="space-between">
        <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
          {t('page.audit.count', { shown: rows.length, total: EVENTS.length })}
        </span>
        <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
          {t('page.audit.realtimeNote')}
        </span>
      </Stack>
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
