import { useState } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

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
          <h1 style={{ margin: 0, fontSize: 24 }}>IoT Auto-Discovery</h1>
          <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
            Scan the local network for OPC-UA, MQTT, Modbus, and EtherNet/IP devices.
            Suggested tag bindings appear next to each match.
          </p>
        </div>
        <Button variant="primary" onClick={startScan} disabled={scanning}>
          {scanning ? 'Scanning…' : 'Scan network'}
        </Button>
      </header>

      {scanning ? (
        <Card>
          <CardBody>
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              Probing /24 with 64-way concurrency · 4 protocol probes · 1.5s/host timeout
            </p>
          </CardBody>
        </Card>
      ) : null}

      {devices.length === 0 && !scanning ? (
        <Card>
          <CardBody>
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              No scan in progress. The skeleton displays demo results when you click "Scan network".
              Real probing wires up in PR #3 alongside the protocol bridges.
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
                {d.bindings} suggested tag bindings · fingerprint <code>{d.fingerprint}</code>
              </span>
              <Button size="sm" disabled>
                Add to inventory (PR #3)
              </Button>
            </Stack>
          </CardBody>
        </Card>
      ))}
    </Stack>
  );
}
