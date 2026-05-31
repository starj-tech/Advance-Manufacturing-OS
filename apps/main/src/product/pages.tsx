import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import type { MachineStatus, WorkOrderStatus } from '@aether/rpc-contracts';
import {
  useAuditLog,
  useInventoryAdjust,
  useMachines,
  useMaterials,
  useWorkOrders,
} from '@aether/data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };
const th: CSSProperties = {
  padding: '10px 16px',
  fontSize: 12,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  textAlign: 'left',
  color: 'var(--aether-fg-muted)',
};
const td: CSSProperties = { padding: '10px 16px' };

const MACHINE_KIND: Record<MachineStatus, StatusKind> = {
  running: 'success',
  idle: 'neutral',
  paused: 'warning',
  fault: 'danger',
  maintenance: 'warning',
  offline: 'neutral',
};
const WO_KIND: Record<WorkOrderStatus, StatusKind> = {
  draft: 'neutral',
  released: 'info',
  running: 'success',
  paused: 'warning',
  completed: 'success',
  canceled: 'danger',
};

export function MachinesPage() {
  const { data: machines = [], isLoading, isError } = useMachines();
  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Mesin</h2>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
          gap: 12,
        }}
      >
        {machines.map((m) => (
          <Card key={m.id}>
            <CardHeader title={m.name} subtitle={m.code} />
            <CardBody>
              <StatusPill kind={MACHINE_KIND[m.status]}>{m.status}</StatusPill>
            </CardBody>
          </Card>
        ))}
      </div>
    </Stack>
  );
}

export function WorkOrdersPage() {
  const { data: orders = [], isLoading, isError } = useWorkOrders();
  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Work Order</h2>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr>
                <th style={th}>Kode</th>
                <th style={th}>Jumlah</th>
                <th style={th}>Status</th>
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
                    <StatusPill kind={WO_KIND[o.status]}>{o.status}</StatusPill>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}

export function MaterialsPage() {
  const { data: materials = [], isLoading, isError } = useMaterials();
  const adjust = useInventoryAdjust();

  const onAdjust = async (id: string, sku: string) => {
    const d = window.prompt(`Delta untuk ${sku} (mis. -10 atau 25):`);
    if (d === null) return;
    const delta = Number(d);
    if (!Number.isFinite(delta) || delta === 0) return;
    const reason = window.prompt('Alasan (opsional):') ?? '';
    try {
      const newQty = await adjust.mutateAsync({
        materialId: id,
        delta,
        reason,
        workOrderId: null,
      });
      if (newQty !== null) window.alert(`${sku} → ${newQty}`);
    } catch (e) {
      window.alert((e as Error).message);
    }
  };

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Inventory</h2>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr>
                <th style={th}>SKU</th>
                <th style={th}>UOM</th>
                <th style={th}>Stok</th>
                <th style={th}>Tercadang</th>
                <th style={th}></th>
              </tr>
            </thead>
            <tbody>
              {materials.map((m) => (
                <tr key={m.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <code>{m.sku}</code>
                  </td>
                  <td style={td}>{m.uom}</td>
                  <td style={td}>{m.qtyOnHand}</td>
                  <td style={td}>{m.qtyReserved}</td>
                  <td style={{ ...td, textAlign: 'right' }}>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={adjust.isPending}
                      onClick={() => void onAdjust(m.id, m.sku)}
                    >
                      Sesuaikan
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
      <p style={muted}>
        Penyesuaian melewati RPC <code>inventory_adjust</code> — atomik, gerbang
        <code> inventory:adjust</code>, dan ter-audit.
      </p>
    </Stack>
  );
}

export function AuditLogPage() {
  const { data: events = [], isLoading, isError } = useAuditLog(50);
  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Audit Log</h2>
      <p style={muted}>
        Ledger append-only per-tenant; auto-refresh tiap 10 detik. Setiap{' '}
        <code>inventory_adjust</code> dari halaman Inventory akan muncul di sini.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
            <thead>
              <tr>
                <th style={th}>Waktu</th>
                <th style={th}>Aksi</th>
                <th style={th}>Resource</th>
                <th style={th}>Detail</th>
              </tr>
            </thead>
            <tbody>
              {events.map((e) => (
                <tr key={e.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{new Date(e.createdAt).toLocaleString()}</td>
                  <td style={td}>
                    <code>{e.action}</code>
                  </td>
                  <td style={td}>
                    {e.resource ?? '—'}
                    {e.resourceId ? ` · ${e.resourceId}` : ''}
                  </td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>
                      {e.metadata ? JSON.stringify(e.metadata) : '—'}
                    </code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}

function Placeholder({ title, note }: { title: string; note: string }) {
  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>{title}</h2>
      <Card>
        <CardBody>
          <p style={muted}>{note}</p>
        </CardBody>
      </Card>
    </Stack>
  );
}

export function OverviewPage() {
  return <Placeholder title="Ringkasan" note="KPI & digital twin per tenant — segera." />;
}
export function CompliancePage() {
  return <Placeholder title="Kepatuhan" note="Laporan kepatuhan bertanda tangan — segera." />;
}
export function TasksPage() {
  return <Placeholder title="Tugas" note="Kartu tugas shop-floor — segera." />;
}
export function ColdChainPage() {
  return (
    <Placeholder title="Rantai Dingin" note="Pemantauan suhu (kapabilitas industri) — segera." />
  );
}
export function TraceabilityPage() {
  return (
    <Placeholder title="Telusur Lot" note="Silsilah lot/batch (kapabilitas industri) — segera." />
  );
}

export const PAGE_COMPONENTS: Record<string, () => JSX.Element> = {
  overview: OverviewPage,
  compliance: CompliancePage,
  'work-orders': WorkOrdersPage,
  machines: MachinesPage,
  materials: MaterialsPage,
  'audit-log': AuditLogPage,
  tasks: TasksPage,
  'cold-chain': ColdChainPage,
  traceability: TraceabilityPage,
};
