import { useState } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

interface Device {
  fingerprint: string;
  host: string;
  port: number;
  probe: 'opc-ua' | 'mqtt' | 'modbus' | 'ethernet-ip';
  vendor: string;
  model: string;
  bindings: number;
}

const DEMO: Device[] = [
  {
    fingerprint: 'aa:bb:cc:dd:ee:01',
    host: '192.168.10.21',
    port: 4840,
    probe: 'opc-ua',
    vendor: 'Siemens',
    model: 'S7-1500 PN/DP',
    bindings: 18,
  },
  {
    fingerprint: 'aa:bb:cc:dd:ee:02',
    host: '192.168.10.34',
    port: 502,
    probe: 'modbus',
    vendor: 'Schneider',
    model: 'Modicon M580',
    bindings: 9,
  },
  {
    fingerprint: 'aa:bb:cc:dd:ee:03',
    host: '192.168.10.42',
    port: 44818,
    probe: 'ethernet-ip',
    vendor: 'Allen-Bradley',
    model: 'CompactLogix 5380',
    bindings: 24,
  },
];

const PILL: Record<Device['probe'], 'info' | 'success' | 'warning' | 'neutral'> = {
  'opc-ua': 'info',
  mqtt: 'success',
  modbus: 'warning',
  'ethernet-ip': 'neutral',
};

export function DiscoveryPage() {
  const { t } = useTranslation();
  const [scanning, setScanning] = useState(false);
  const [devices, setDevices] = useState<Device[]>([]);

  const startScan = () => {
    setScanning(true);
    setDevices([]);
    // PR #3: invoke('discovery_scan', { cidr: '192.168.10.0/24' })
    setTimeout(() => {
      setDevices(DEMO);
      setScanning(false);
    }, 1500);
  };

  return (
    <Stack gap={16}>
      <header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div>
          <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.discovery.title')}</h1>
          <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
            {t('page.discovery.intro')}
          </p>
        </div>
        <Button variant="primary" onClick={startScan} disabled={scanning}>
          {scanning ? t('page.discovery.scanning') : t('page.discovery.scan')}
        </Button>
      </header>

      {scanning ? (
        <Card>
          <CardBody>
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              {t('page.discovery.scanningNote')}
            </p>
          </CardBody>
        </Card>
      ) : null}

      {devices.length === 0 && !scanning ? (
        <Card>
          <CardBody>
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              {t('page.discovery.emptyNote')}
            </p>
          </CardBody>
        </Card>
      ) : null}

      {devices.map((d) => (
        <Card key={d.fingerprint}>
          <CardHeader title={`${d.vendor} · ${d.model}`} subtitle={`${d.host}:${d.port}`}>
            <div style={{ position: 'absolute', top: 16, right: 16 }}>
              <StatusPill kind={PILL[d.probe]}>{d.probe}</StatusPill>
            </div>
          </CardHeader>
          <CardBody>
            <Stack direction="row" align="center" justify="space-between">
              <span style={{ fontSize: 13, color: 'var(--aether-fg-muted)' }}>
                {t('page.discovery.bindings', { count: d.bindings })} <code>{d.fingerprint}</code>
              </span>
              <Button size="sm" disabled>
                {t('page.discovery.addToInventory')}
              </Button>
            </Stack>
          </CardBody>
        </Card>
      ))}
    </Stack>
  );
}
