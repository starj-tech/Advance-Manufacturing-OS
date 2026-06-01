import { useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import {
  PROTOCOL_FAMILIES,
  PROTOCOL_PATH,
  useUpsertMachine,
  type DeviceBinding,
  type MachineStatusLite,
  type ProtocolFamily,
} from '@aether/data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };
const labelStyle: CSSProperties = {
  display: 'block',
  fontSize: 12,
  color: 'var(--aether-fg-muted)',
  marginBottom: 6,
};
const field: CSSProperties = {
  width: '100%',
  padding: '10px 12px',
  fontSize: 14,
  background: 'var(--aether-bg)',
  color: 'var(--aether-fg)',
  border: '1px solid var(--aether-border)',
  borderRadius: 8,
  outline: 'none',
};

const STATUSES: MachineStatusLite[] = [
  'running',
  'idle',
  'paused',
  'fault',
  'maintenance',
  'offline',
];

interface ProtocolField {
  key: string;
  label: string;
  placeholder?: string;
  required?: boolean;
}

/** Per-protocol field schema. Empty array = no extra fields needed. */
const PROTOCOL_FIELDS: Record<ProtocolFamily, ProtocolField[]> = {
  opcua: [
    {
      key: 'endpoint',
      label: 'Endpoint OPC-UA',
      placeholder: 'opc.tcp://10.0.0.21:4840',
      required: true,
    },
    { key: 'node_id', label: 'Node ID', placeholder: 'ns=2;s=Press.State', required: true },
    { key: 'security_policy', label: 'Security policy', placeholder: 'None / Basic256Sha256' },
  ],
  mqtt: [
    {
      key: 'endpoint',
      label: 'Broker (TCP)',
      placeholder: 'mqtt://10.0.0.34:1883',
      required: true,
    },
    { key: 'topic', label: 'Topik', placeholder: 'cnc/12/state', required: true },
    { key: 'client_id', label: 'Client ID', placeholder: 'aether-cnc-12' },
  ],
  'mqtt-ws': [
    {
      key: 'endpoint',
      label: 'Broker (WebSocket)',
      placeholder: 'wss://broker.contoh.co:8884',
      required: true,
    },
    { key: 'topic', label: 'Topik', placeholder: 'plant/A/line-1/state', required: true },
  ],
  'modbus-tcp': [
    { key: 'endpoint', label: 'Endpoint', placeholder: '10.0.0.50:502', required: true },
    { key: 'unit_id', label: 'Unit ID', placeholder: '1' },
    { key: 'register', label: 'Register awal', placeholder: '40001', required: true },
    { key: 'count', label: 'Jumlah register', placeholder: '10' },
  ],
  'modbus-rtu': [
    { key: 'serial_port', label: 'Serial port (gateway-side)', placeholder: '/dev/ttyUSB0' },
    { key: 'baud', label: 'Baud rate', placeholder: '9600' },
    { key: 'unit_id', label: 'Unit ID', placeholder: '1' },
    { key: 'register', label: 'Register awal', placeholder: '40001', required: true },
  ],
  'http-poll': [
    {
      key: 'endpoint',
      label: 'URL',
      placeholder: 'https://machine.local/api/state',
      required: true,
    },
    { key: 'interval_s', label: 'Polling (detik)', placeholder: '5' },
    { key: 'auth_header', label: 'Header autentikasi (opsional)', placeholder: 'Bearer …' },
  ],
  websocket: [
    {
      key: 'endpoint',
      label: 'URL WebSocket',
      placeholder: 'wss://machine.contoh.co/stream',
      required: true,
    },
  ],
  'web-serial': [
    { key: 'baud', label: 'Baud rate', placeholder: '115200' },
    { key: 'parity', label: 'Parity', placeholder: 'none / even / odd' },
    { key: 'data_bits', label: 'Data bits', placeholder: '8' },
    { key: 'stop_bits', label: 'Stop bits', placeholder: '1' },
  ],
  'web-usb': [
    { key: 'vendor_id', label: 'Vendor ID (hex)', placeholder: '0x1234' },
    { key: 'product_id', label: 'Product ID (hex)', placeholder: '0x5678' },
    { key: 'interface_number', label: 'Interface', placeholder: '0' },
  ],
  'web-bluetooth': [
    {
      key: 'service_uuid',
      label: 'Service UUID',
      placeholder: '0000180a-0000-1000-8000-00805f9b34fb',
    },
    { key: 'characteristic_uuid', label: 'Characteristic UUID' },
    { key: 'name_prefix', label: 'Name prefix (opsional)', placeholder: 'SENSOR-' },
  ],
  'web-hid': [
    { key: 'vendor_id', label: 'Vendor ID (hex)', placeholder: '0x05ac' },
    { key: 'product_id', label: 'Product ID (hex)', placeholder: '0x024f' },
  ],
  manual: [{ key: 'notes', label: 'Catatan', placeholder: 'Bagaimana data masuk?' }],
  'file-upload': [
    { key: 'expected_format', label: 'Format file', placeholder: 'csv / json / xml' },
    { key: 'columns', label: 'Kolom yang dipakai', placeholder: 'timestamp,value,tag_id' },
  ],
};

const PATH_KIND = {
  gateway: 'info',
  'browser-direct': 'success',
  misc: 'neutral',
} as const;

const PATH_LABEL = {
  gateway: 'lewat gateway',
  'browser-direct': 'langsung browser',
  misc: 'manual',
} as const;

export function MachineBindingDialog({
  initial,
  onClose,
}: {
  initial?: DeviceBinding;
  onClose: () => void;
}) {
  const editing = initial != null;
  const [code, setCode] = useState(initial?.code ?? '');
  const [name, setName] = useState(initial?.name ?? '');
  const [status, setStatus] = useState<MachineStatusLite>(
    (initial?.status as MachineStatusLite) || 'idle',
  );
  const [protocol, setProtocol] = useState<ProtocolFamily>(
    (initial?.protocol as ProtocolFamily) || 'opcua',
  );
  const initBinding = useMemo(() => {
    const b: Record<string, string> = {};
    for (const [k, v] of Object.entries(initial?.binding ?? {})) {
      if (k === 'protocol') continue;
      b[k] = v == null ? '' : String(v);
    }
    return b;
  }, [initial]);
  const [binding, setBinding] = useState<Record<string, string>>(initBinding);
  const [errorMsg, setErrorMsg] = useState('');

  const upsert = useUpsertMachine();
  const fields = PROTOCOL_FIELDS[protocol];

  const submit = async () => {
    setErrorMsg('');
    try {
      // Trim empties so the binding JSONB stays clean.
      const cleaned: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(binding)) {
        if (v != null && String(v).trim().length > 0) cleaned[k] = v;
      }
      await upsert.mutateAsync({
        id: initial?.machineId ?? null,
        code: code.trim(),
        name: name.trim(),
        status,
        protocol,
        binding: cleaned,
      });
      onClose();
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  };

  const path = PROTOCOL_PATH[protocol];
  const requiredOk = fields.every((f) => !f.required || (binding[f.key] ?? '').trim().length > 0);
  const canSubmit =
    code.trim().length > 0 && name.trim().length > 0 && requiredOk && !upsert.isPending;

  return (
    <Card>
      <CardHeader
        title={editing ? 'Edit binding mesin' : 'Tambah mesin & binding'}
        subtitle="Pilih jalur integrasi — gateway on-prem atau langsung dari browser"
      />
      <CardBody>
        <Stack gap={12}>
          <Stack direction="row" gap={12} align="center">
            <div style={{ flex: 1 }}>
              <label style={labelStyle}>Kode</label>
              <input
                style={field}
                value={code}
                onChange={(e) => setCode(e.target.value)}
                placeholder="PRESS-02"
                disabled={editing}
              />
            </div>
            <div style={{ flex: 2 }}>
              <label style={labelStyle}>Nama</label>
              <input
                style={field}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Hydraulic press 250t (line 2)"
              />
            </div>
          </Stack>

          <Stack direction="row" gap={12} align="center">
            <div style={{ flex: 1 }}>
              <label style={labelStyle}>Status awal</label>
              <select
                style={field}
                value={status}
                onChange={(e) => setStatus(e.target.value as MachineStatusLite)}
              >
                {STATUSES.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </div>
            <div style={{ flex: 2 }}>
              <label style={labelStyle}>Protokol</label>
              <select
                style={field}
                value={protocol}
                onChange={(e) => {
                  setProtocol(e.target.value as ProtocolFamily);
                  setBinding({});
                }}
              >
                {PROTOCOL_FAMILIES.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </div>
          </Stack>

          <Stack direction="row" gap={6} align="center">
            <StatusPill kind={PATH_KIND[path]}>{PATH_LABEL[path]}</StatusPill>
            <p style={muted}>
              {path === 'gateway'
                ? 'Koneksi dijalankan oleh gateway on-prem; aplikasi web hanya menyimpan konfigurasinya.'
                : path === 'browser-direct'
                  ? 'Browser membuka koneksi sendiri saat operator mengakses halaman (butuh izin & secure context).'
                  : 'Tidak ada koneksi otomatis; data masuk lewat input manual atau upload file.'}
            </p>
          </Stack>

          {fields.length > 0 ? (
            <Card>
              <CardBody>
                <Stack gap={8}>
                  {fields.map((f) => (
                    <div key={f.key}>
                      <label style={labelStyle}>
                        {f.label}
                        {f.required ? ' *' : ''}
                      </label>
                      <input
                        style={field}
                        value={binding[f.key] ?? ''}
                        onChange={(e) => setBinding({ ...binding, [f.key]: e.target.value })}
                        placeholder={f.placeholder}
                      />
                    </div>
                  ))}
                </Stack>
              </CardBody>
            </Card>
          ) : null}

          {errorMsg ? <StatusPill kind="danger">{errorMsg}</StatusPill> : null}

          <Stack direction="row" gap={8}>
            <Button variant="primary" size="sm" disabled={!canSubmit} onClick={() => void submit()}>
              {upsert.isPending ? 'Menyimpan…' : editing ? 'Simpan perubahan' : 'Tambahkan'}
            </Button>
            <Button variant="ghost" size="sm" onClick={onClose}>
              Batal
            </Button>
          </Stack>
        </Stack>
      </CardBody>
    </Card>
  );
}
