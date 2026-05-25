import { useState } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

interface Module {
  id: string;
  name: string;
  version: string;
  publisher: string;
  signed: boolean;
  installed: boolean;
  enabled: boolean;
}

// Demo catalog; PR #4 fetches the real registry from Supabase Storage,
// verifies ed25519 signatures, and honors the kill-switch.
const SEED: Module[] = [
  {
    id: 'com.aether.cold-chain',
    name: 'Cold Chain Monitor',
    version: '1.4.2',
    publisher: 'AETHER Labs',
    signed: true,
    installed: true,
    enabled: true,
  },
  {
    id: 'com.aether.haccp',
    name: 'HACCP Control Points',
    version: '0.9.0',
    publisher: 'AETHER Labs',
    signed: true,
    installed: true,
    enabled: true,
  },
  {
    id: 'com.aether.ppap',
    name: 'PPAP & Control Plan',
    version: '2.1.0',
    publisher: 'AETHER Labs',
    signed: true,
    installed: true,
    enabled: false,
  },
  {
    id: 'com.aether.spc',
    name: 'Statistical Process Control',
    version: '1.0.3',
    publisher: 'AETHER Labs',
    signed: true,
    installed: false,
    enabled: false,
  },
  {
    id: 'com.acme.oee-pro',
    name: 'OEE Pro',
    version: '3.2.1',
    publisher: 'Acme Analytics',
    signed: false,
    installed: false,
    enabled: false,
  },
  {
    id: 'com.aether.predictive-maintenance',
    name: 'Predictive Maintenance',
    version: '0.5.0',
    publisher: 'AETHER Labs',
    signed: true,
    installed: false,
    enabled: false,
  },
];

export function ModuleRegistryPage() {
  const { t } = useTranslation();
  const [modules, setModules] = useState<Module[]>(SEED);

  const update = (id: string, patch: Partial<Module>) =>
    setModules((ms) => ms.map((m) => (m.id === id ? { ...m, ...patch } : m)));

  const installed = modules.filter((m) => m.installed);
  const available = modules.filter((m) => !m.installed);

  const sigPill = (m: Module) =>
    m.signed ? (
      <StatusPill kind="success">{t('page.modules.sig.verified')}</StatusPill>
    ) : (
      <StatusPill kind="warning">{t('page.modules.sig.unsigned')}</StatusPill>
    );

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.modules.title')}</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          {t('page.modules.desc')}
        </p>
      </header>

      <Card padded={false}>
        <CardHeader title={t('page.modules.installedTitle')} />
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                <th style={th}>{t('page.modules.col.module')}</th>
                <th style={th}>{t('page.modules.col.version')}</th>
                <th style={th}>{t('page.modules.col.publisher')}</th>
                <th style={th}>{t('page.modules.col.signature')}</th>
                <th style={th} />
              </tr>
            </thead>
            <tbody>
              {installed.map((m) => (
                <tr key={m.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <div style={{ fontWeight: 600 }}>{m.name}</div>
                    <code style={{ fontSize: 11, color: 'var(--aether-fg-muted)' }}>{m.id}</code>
                  </td>
                  <td style={td}>{m.version}</td>
                  <td style={td}>{m.publisher}</td>
                  <td style={td}>
                    <Stack direction="row" gap={6} align="center">
                      {sigPill(m)}
                      <StatusPill kind={m.enabled ? 'info' : 'neutral'}>
                        {t(
                          m.enabled ? 'page.modules.state.enabled' : 'page.modules.state.disabled',
                        )}
                      </StatusPill>
                    </Stack>
                  </td>
                  <td style={{ ...td, textAlign: 'right' }}>
                    <Stack direction="row" gap={6} justify="flex-end">
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => update(m.id, { enabled: !m.enabled })}
                      >
                        {t(m.enabled ? 'page.modules.disable' : 'page.modules.enable')}
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => update(m.id, { installed: false, enabled: false })}
                      >
                        {t('page.modules.revoke')}
                      </Button>
                    </Stack>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>

      <Card padded={false}>
        <CardHeader title={t('page.modules.availableTitle')} />
        <CardBody>
          {available.length === 0 ? (
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              {t('page.modules.emptyAvailable')}
            </p>
          ) : (
            <Stack gap={8}>
              {available.map((m) => (
                <Stack
                  key={m.id}
                  direction="row"
                  align="center"
                  justify="space-between"
                  style={{ padding: '8px 0', borderTop: '1px solid var(--aether-border)' }}
                >
                  <div>
                    <div style={{ fontWeight: 600 }}>
                      {m.name}{' '}
                      <span style={{ color: 'var(--aether-fg-muted)' }}>· {m.version}</span>
                    </div>
                    <code style={{ fontSize: 11, color: 'var(--aether-fg-muted)' }}>{m.id}</code>
                  </div>
                  <Stack direction="row" gap={8} align="center">
                    {sigPill(m)}
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={() => update(m.id, { installed: true, enabled: true })}
                    >
                      {t('page.modules.install')}
                    </Button>
                  </Stack>
                </Stack>
              ))}
            </Stack>
          )}
        </CardBody>
      </Card>

      <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
        {t('page.modules.note')}
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
