/**
 * Per-protocol "driver" cards. Each protocol family has its own React
 * component that knows how to render the live UI for a machine bound on
 * that protocol.
 *
 * The browser-direct families (Web Serial / Web USB / Web Bluetooth /
 * Web HID / WebSocket / mqtt-ws) open a real connection from the browser
 * when the operator clicks "Hubungkan". The gateway-mediated families
 * just describe what the on-prem gateway should be doing and render the
 * latest telemetry that's already in our DB tables.
 *
 * Modules can be registered for additional protocols by adding entries
 * to MACHINE_DRIVERS below.
 */

import { useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import type { DeviceBinding, ProtocolFamily } from '@aether/data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };
const mono: CSSProperties = {
  fontFamily: 'monospace',
  fontSize: 12,
  background: 'var(--aether-bg)',
  padding: '6px 8px',
  borderRadius: 6,
  border: '1px solid var(--aether-border)',
  maxHeight: 120,
  overflow: 'auto',
  whiteSpace: 'pre-wrap',
};

export interface DriverProps {
  binding: DeviceBinding;
}

// ============================================================================
// Web Serial driver — REAL browser-direct connection to a USB-serial device.
// ============================================================================

type SerialNavigator = Navigator & {
  serial?: {
    requestPort: (opts?: {
      filters?: Array<{ usbVendorId?: number; usbProductId?: number }>;
    }) => Promise<SerialPort>;
  };
};
interface SerialPort {
  open: (opts: {
    baudRate: number;
    dataBits?: number;
    stopBits?: number;
    parity?: string;
  }) => Promise<void>;
  close: () => Promise<void>;
  readable: ReadableStream<Uint8Array> | null;
}

function WebSerialCard({ binding }: DriverProps) {
  const [status, setStatus] = useState<'idle' | 'requesting' | 'connected' | 'error'>('idle');
  const [error, setError] = useState<string>('');
  const [lines, setLines] = useState<string[]>([]);
  const abortRef = useRef<AbortController | null>(null);

  const supported =
    typeof navigator !== 'undefined' && (navigator as SerialNavigator).serial != null;

  const connect = async () => {
    setStatus('requesting');
    setError('');
    try {
      const port = await (navigator as SerialNavigator).serial!.requestPort();
      const baud = Number(binding.binding.baud ?? 115200);
      await port.open({
        baudRate: baud,
        dataBits: Number(binding.binding.data_bits ?? 8),
        stopBits: Number(binding.binding.stop_bits ?? 1),
        parity: (binding.binding.parity as string) || 'none',
      });
      setStatus('connected');
      const ctrl = new AbortController();
      abortRef.current = ctrl;
      void readLoop(port, ctrl.signal, (line) => setLines((prev) => [line, ...prev].slice(0, 50)));
    } catch (e) {
      setStatus('error');
      setError((e as Error).message);
    }
  };

  useEffect(() => () => abortRef.current?.abort(), []);

  if (!supported) {
    return (
      <Stack gap={6}>
        <StatusPill kind="warning">browser tidak mendukung Web Serial</StatusPill>
        <p style={muted}>Butuh Chrome/Edge dengan HTTPS atau localhost.</p>
      </Stack>
    );
  }
  return (
    <Stack gap={8}>
      <Stack direction="row" gap={6} align="center">
        <Button
          size="sm"
          variant={status === 'connected' ? 'ghost' : 'primary'}
          disabled={status === 'requesting' || status === 'connected'}
          onClick={() => void connect()}
        >
          {status === 'connected' ? 'Tersambung' : 'Hubungkan ke port serial'}
        </Button>
        {status === 'connected' ? <StatusPill kind="success">live</StatusPill> : null}
        {status === 'error' ? <StatusPill kind="danger">{error}</StatusPill> : null}
      </Stack>
      <p style={muted}>
        baud {String(binding.binding.baud ?? 115200)} · parity{' '}
        {(binding.binding.parity as string) || 'none'}
      </p>
      {lines.length > 0 ? (
        <div style={mono}>
          {lines.map((l, i) => (
            <div key={i}>{l}</div>
          ))}
        </div>
      ) : (
        <p style={muted}>Klik Hubungkan dan pilih port USB-serial dari prompt browser.</p>
      )}
    </Stack>
  );
}

async function readLoop(
  port: SerialPort,
  signal: AbortSignal,
  onLine: (line: string) => void,
): Promise<void> {
  if (!port.readable) return;
  const reader = port.readable.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  try {
    while (!signal.aborted) {
      const { value, done } = await reader.read();
      if (done) break;
      if (value) {
        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split(/\r?\n/);
        buffer = lines.pop() ?? '';
        for (const line of lines) if (line.length > 0) onLine(line);
      }
    }
  } catch (e) {
    onLine(`[error] ${(e as Error).message}`);
  } finally {
    try {
      reader.releaseLock();
    } catch {
      /* ignore */
    }
  }
}

// ============================================================================
// WebSocket driver — REAL browser-direct WS connection.
// ============================================================================

function WebSocketCard({ binding }: DriverProps) {
  const endpoint = (binding.binding.endpoint as string) ?? '';
  const [status, setStatus] = useState<'idle' | 'connecting' | 'open' | 'closed' | 'error'>('idle');
  const [error, setError] = useState('');
  const [messages, setMessages] = useState<string[]>([]);
  const wsRef = useRef<WebSocket | null>(null);

  const connect = () => {
    if (!endpoint) return;
    setStatus('connecting');
    setError('');
    try {
      const ws = new WebSocket(endpoint);
      wsRef.current = ws;
      ws.onopen = () => setStatus('open');
      ws.onerror = () => {
        setStatus('error');
        setError('koneksi gagal');
      };
      ws.onclose = () => setStatus('closed');
      ws.onmessage = (ev) =>
        setMessages((prev) =>
          [typeof ev.data === 'string' ? ev.data : '[binary]', ...prev].slice(0, 50),
        );
    } catch (e) {
      setStatus('error');
      setError((e as Error).message);
    }
  };

  useEffect(() => () => wsRef.current?.close(), []);

  return (
    <Stack gap={8}>
      <Stack direction="row" gap={6} align="center">
        <Button
          size="sm"
          variant={status === 'open' ? 'ghost' : 'primary'}
          disabled={!endpoint || status === 'connecting' || status === 'open'}
          onClick={connect}
        >
          {status === 'open' ? 'Tersambung' : 'Hubungkan'}
        </Button>
        {status === 'open' ? <StatusPill kind="success">live</StatusPill> : null}
        {status === 'error' ? <StatusPill kind="danger">{error}</StatusPill> : null}
      </Stack>
      <p style={muted}>{endpoint || 'endpoint belum diset di binding'}</p>
      {messages.length > 0 ? (
        <div style={mono}>
          {messages.map((m, i) => (
            <div key={i}>{m}</div>
          ))}
        </div>
      ) : null}
    </Stack>
  );
}

// ============================================================================
// Web Bluetooth driver — REAL browser-direct GATT scan.
// ============================================================================

type BTNavigator = Navigator & {
  bluetooth?: {
    requestDevice: (opts: {
      filters?: Array<{ services?: string[]; namePrefix?: string }>;
      optionalServices?: string[];
      acceptAllDevices?: boolean;
    }) => Promise<BluetoothDevice>;
  };
};
interface BluetoothDevice {
  name?: string;
  id: string;
  gatt?: { connect: () => Promise<unknown>; disconnect: () => void };
}

function WebBluetoothCard({ binding }: DriverProps) {
  const [status, setStatus] = useState<'idle' | 'requesting' | 'connected' | 'error'>('idle');
  const [error, setError] = useState('');
  const [deviceName, setDeviceName] = useState('');
  const supported =
    typeof navigator !== 'undefined' && (navigator as BTNavigator).bluetooth != null;

  const connect = async () => {
    setStatus('requesting');
    setError('');
    const serviceUuid = (binding.binding.service_uuid as string) || undefined;
    const namePrefix = (binding.binding.name_prefix as string) || undefined;
    try {
      const device = await (navigator as BTNavigator).bluetooth!.requestDevice({
        filters:
          serviceUuid || namePrefix
            ? [{ services: serviceUuid ? [serviceUuid] : undefined, namePrefix }]
            : undefined,
        acceptAllDevices: !serviceUuid && !namePrefix,
        optionalServices: serviceUuid ? [serviceUuid] : [],
      });
      setDeviceName(device.name ?? device.id);
      await device.gatt?.connect();
      setStatus('connected');
    } catch (e) {
      setStatus('error');
      setError((e as Error).message);
    }
  };

  if (!supported) {
    return (
      <Stack gap={6}>
        <StatusPill kind="warning">browser tidak mendukung Web Bluetooth</StatusPill>
        <p style={muted}>Butuh Chrome/Edge dengan secure context (HTTPS).</p>
      </Stack>
    );
  }
  return (
    <Stack gap={8}>
      <Stack direction="row" gap={6} align="center">
        <Button
          size="sm"
          variant={status === 'connected' ? 'ghost' : 'primary'}
          disabled={status === 'requesting' || status === 'connected'}
          onClick={() => void connect()}
        >
          {status === 'connected' ? `Tersambung: ${deviceName}` : 'Pasangkan perangkat BLE'}
        </Button>
        {status === 'error' ? <StatusPill kind="danger">{error}</StatusPill> : null}
      </Stack>
      <p style={muted}>
        Service: <code>{(binding.binding.service_uuid as string) || '—'}</code>
      </p>
    </Stack>
  );
}

// ============================================================================
// Web USB / Web HID — REAL browser-direct enumeration.
// ============================================================================

type WebUsbNavigator = Navigator & {
  usb?: { requestDevice: (opts: { filters: Array<Record<string, number>> }) => Promise<unknown> };
};
type WebHidNavigator = Navigator & {
  hid?: { requestDevice: (opts: { filters: Array<Record<string, number>> }) => Promise<unknown[]> };
};

function WebUsbLikeCard({ binding, kind }: DriverProps & { kind: 'web-usb' | 'web-hid' }) {
  const [status, setStatus] = useState<'idle' | 'connected' | 'error'>('idle');
  const [error, setError] = useState('');
  const supported =
    kind === 'web-usb'
      ? (navigator as WebUsbNavigator).usb != null
      : (navigator as WebHidNavigator).hid != null;

  const connect = async () => {
    try {
      const vid = parseInt(String(binding.binding.vendor_id ?? '0'), 16);
      const pid = parseInt(String(binding.binding.product_id ?? '0'), 16);
      const filters = [{ vendorId: vid || 0, productId: pid || 0 }].filter((f) => f.vendorId > 0);
      if (kind === 'web-usb') {
        await (navigator as WebUsbNavigator).usb!.requestDevice({
          filters: filters.length > 0 ? filters : [{}],
        });
      } else {
        await (navigator as WebHidNavigator).hid!.requestDevice({
          filters: filters.length > 0 ? filters : [{}],
        });
      }
      setStatus('connected');
    } catch (e) {
      setStatus('error');
      setError((e as Error).message);
    }
  };

  if (!supported) return <StatusPill kind="warning">browser tidak mendukung {kind}</StatusPill>;
  return (
    <Stack gap={8}>
      <Stack direction="row" gap={6} align="center">
        <Button
          size="sm"
          variant={status === 'connected' ? 'ghost' : 'primary'}
          disabled={status === 'connected'}
          onClick={() => void connect()}
        >
          {status === 'connected' ? 'Tersambung' : 'Pilih perangkat'}
        </Button>
        {status === 'error' ? <StatusPill kind="danger">{error}</StatusPill> : null}
      </Stack>
      <p style={muted}>
        VID/PID: {(binding.binding.vendor_id as string) || '—'} /{' '}
        {(binding.binding.product_id as string) || '—'}
      </p>
    </Stack>
  );
}

// ============================================================================
// Gateway-mediated families — render telemetry from the DB instead of opening
// a direct connection (browsers can't reach factory-floor PLCs directly).
// ============================================================================

function GatewayPlaceholder({ binding }: DriverProps) {
  return (
    <Stack gap={6}>
      <StatusPill kind="info">via gateway on-prem</StatusPill>
      <p style={muted}>
        Endpoint: <code>{binding.endpoint ?? '—'}</code>
      </p>
      <p style={muted}>
        Gateway daemon menarik data dan menulis ke <code>gateway_samples</code>; aplikasi web
        mendengarkan via Supabase Realtime atau polling.
      </p>
    </Stack>
  );
}

// ============================================================================
// Manual / file-upload — no connection, just metadata.
// ============================================================================

function ManualCard({ binding }: DriverProps) {
  return (
    <Stack gap={6}>
      <StatusPill kind="neutral">input manual</StatusPill>
      <p style={muted}>
        {(binding.binding.notes as string) || 'Data dimasukkan tangan / upload file.'}
      </p>
    </Stack>
  );
}

// ============================================================================
// Registry.
// ============================================================================

export const MACHINE_DRIVERS: Record<ProtocolFamily, (props: DriverProps) => JSX.Element> = {
  opcua: GatewayPlaceholder,
  mqtt: GatewayPlaceholder,
  'modbus-tcp': GatewayPlaceholder,
  'modbus-rtu': GatewayPlaceholder,
  'http-poll': GatewayPlaceholder,
  'mqtt-ws': (p) => <WebSocketCard {...p} />,
  websocket: (p) => <WebSocketCard {...p} />,
  'web-serial': WebSerialCard,
  'web-usb': (p) => <WebUsbLikeCard {...p} kind="web-usb" />,
  'web-hid': (p) => <WebUsbLikeCard {...p} kind="web-hid" />,
  'web-bluetooth': WebBluetoothCard,
  manual: ManualCard,
  'file-upload': ManualCard,
};

export function MachineLiveCard({ binding }: { binding: DeviceBinding }) {
  const protocol = (binding.protocol as ProtocolFamily) || 'manual';
  const Driver = MACHINE_DRIVERS[protocol] ?? GatewayPlaceholder;
  const statusKind: StatusKind =
    binding.status === 'running'
      ? 'success'
      : binding.status === 'fault'
        ? 'danger'
        : binding.status === 'paused' || binding.status === 'maintenance'
          ? 'warning'
          : 'neutral';
  return (
    <Card>
      <CardHeader title={binding.name} subtitle={binding.code} />
      <CardBody>
        <Stack gap={10}>
          <Stack direction="row" gap={6} align="center">
            <StatusPill kind={statusKind}>{binding.status || 'unknown'}</StatusPill>
            <StatusPill kind="info">{protocol}</StatusPill>
          </Stack>
          <Driver binding={binding} />
        </Stack>
      </CardBody>
    </Card>
  );
}
