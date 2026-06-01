import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import type { MachineStatus, WorkOrderStatus } from '@aether/rpc-contracts';
import {
  useAuditLog,
  useColdChainReadings,
  useComplianceReports,
  useInventoryAdjust,
  useLots,
  useMachines,
  useMaterials,
  useWorkOrderAdvance,
  useWorkOrders,
} from '@aether/data';
import type { ComplianceStatus, Lot, TemperatureReading } from '@aether/data';

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

const WO_NEXT: Record<WorkOrderStatus, ReadonlyArray<{ label: string; to: WorkOrderStatus }>> = {
  draft: [
    { label: 'Rilis', to: 'released' },
    { label: 'Batal', to: 'canceled' },
  ],
  released: [
    { label: 'Mulai', to: 'running' },
    { label: 'Batal', to: 'canceled' },
  ],
  running: [
    { label: 'Jeda', to: 'paused' },
    { label: 'Selesai', to: 'completed' },
  ],
  paused: [
    { label: 'Lanjut', to: 'running' },
    { label: 'Batal', to: 'canceled' },
  ],
  completed: [],
  canceled: [],
};

export function WorkOrdersPage() {
  const { data: orders = [], isLoading, isError } = useWorkOrders();
  const advance = useWorkOrderAdvance();

  const onAdvance = async (id: string, to: WorkOrderStatus, expectedHlc: string) => {
    try {
      await advance.mutateAsync({ id, to, expectedHlc });
    } catch (e) {
      window.alert((e as Error).message);
    }
  };

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
                <th style={th}></th>
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
                  <td style={{ ...td, textAlign: 'right' }}>
                    <span style={{ display: 'inline-flex', gap: 6 }}>
                      {WO_NEXT[o.status].map((n) => (
                        <Button
                          key={n.to}
                          size="sm"
                          variant="ghost"
                          disabled={advance.isPending}
                          onClick={() => void onAdvance(o.id, n.to, o.hlc)}
                        >
                          {n.label}
                        </Button>
                      ))}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
      <p style={muted}>
        Transisi melewati RPC <code>work_order_advance</code> — state machine atomik, gerbang{' '}
        <code>work_orders:update</code>, dan ter-audit.
      </p>
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

function Kpi({
  label,
  value,
  hint,
  kind,
}: {
  label: string;
  value: string;
  hint?: string;
  kind?: StatusKind;
}) {
  return (
    <Card>
      <CardBody>
        <Stack gap={6}>
          <p style={{ ...muted, margin: 0 }}>{label}</p>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 10 }}>
            <span style={{ fontSize: 28, fontWeight: 600 }}>{value}</span>
            {kind ? <StatusPill kind={kind}>{hint ?? ''}</StatusPill> : null}
          </div>
          {!kind && hint ? <p style={{ ...muted, margin: 0 }}>{hint}</p> : null}
        </Stack>
      </CardBody>
    </Card>
  );
}

export function OverviewPage() {
  const { data: machines = [] } = useMachines();
  const { data: orders = [] } = useWorkOrders();
  const { data: materials = [] } = useMaterials();
  const { data: readings = [] } = useColdChainReadings(240);
  const { data: events = [] } = useAuditLog(20);

  const running = machines.filter((m) => m.status === 'running').length;
  const fault = machines.filter((m) => m.status === 'fault').length;
  const openOrders = orders.filter((o) => o.status !== 'completed' && o.status !== 'canceled');
  const lowStock = materials.filter((m) => m.qtyOnHand <= m.qtyReserved + 10).length;

  const dayAgo = Date.now() - 24 * 3_600_000;
  const breaches = readings.filter(
    (r) =>
      new Date(r.takenAt).getTime() >= dayAgo &&
      ((r.lowC != null && r.celsius < r.lowC) || (r.highC != null && r.celsius > r.highC)),
  ).length;

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Ringkasan</h2>
      <p style={muted}>
        KPI per tenant — angka di sini tertarik langsung dari tabel produksi dan jejak audit.
      </p>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 12,
        }}
      >
        <Kpi
          label="Mesin aktif"
          value={`${running} / ${machines.length}`}
          hint={fault > 0 ? `${fault} fault` : 'no fault'}
          kind={fault > 0 ? 'danger' : 'success'}
        />
        <Kpi
          label="Work order terbuka"
          value={String(openOrders.length)}
          hint={
            orders.length === 0
              ? '—'
              : `${orders.filter((o) => o.status === 'running').length} running · ${orders.filter((o) => o.status === 'paused').length} paused`
          }
        />
        <Kpi
          label="Material stok rendah"
          value={String(lowStock)}
          hint={lowStock === 0 ? 'aman' : 'butuh PO'}
          kind={lowStock === 0 ? 'success' : 'warning'}
        />
        <Kpi
          label="Pelanggaran rantai dingin (24 jam)"
          value={String(breaches)}
          hint={breaches === 0 ? 'normal' : 'investigasi'}
          kind={breaches === 0 ? 'success' : 'danger'}
        />
      </div>
      <Card>
        <CardHeader title="Aktivitas terakhir" subtitle="Sumber: audit_log" />
        <CardBody>
          {events.length === 0 ? (
            <p style={muted}>Belum ada aktivitas.</p>
          ) : (
            <Stack gap={6}>
              {events.slice(0, 10).map((e) => (
                <div
                  key={e.id}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    fontSize: 13,
                    color: 'var(--aether-fg)',
                  }}
                >
                  <span>
                    <code>{e.action}</code>
                    {e.resource ? ` · ${e.resource}` : ''}
                  </span>
                  <span style={{ color: 'var(--aether-fg-muted)', fontSize: 12 }}>
                    {new Date(e.createdAt).toLocaleString()}
                  </span>
                </div>
              ))}
            </Stack>
          )}
        </CardBody>
      </Card>
    </Stack>
  );
}
const COMPLIANCE_KIND: Record<ComplianceStatus, StatusKind> = {
  compliant: 'success',
  'needs-review': 'warning',
  'non-compliant': 'danger',
};

export function CompliancePage() {
  const { data: reports = [], isLoading, isError } = useComplianceReports(20);

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Kepatuhan</h2>
      <p style={muted}>
        Laporan kepatuhan bertanda tangan Edge Function. Setiap baris adalah snapshot probe kontrol
        terhadap standar yang terdaftar (HACCP, ISO-22000, BPOM). Bukti dirangkum di{' '}
        <code>compliance_evidence</code> via hash chain.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr>
                <th style={th}>Standar</th>
                <th style={th}>Status</th>
                <th style={th}>Kontrol</th>
                <th style={th}>Dibuat</th>
                <th style={th}>Tanda tangan</th>
              </tr>
            </thead>
            <tbody>
              {reports.map((r) => (
                <tr key={r.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <code>{r.standardSlug}</code>
                  </td>
                  <td style={td}>
                    <StatusPill kind={COMPLIANCE_KIND[r.status]}>{r.status}</StatusPill>
                  </td>
                  <td style={td}>
                    {r.controlsPassed}/{r.controlsTotal} lulus
                    {r.controlsFailed > 0 ? ` · ${r.controlsFailed} gagal` : ''}
                    {r.controlsNeedsReview > 0 ? ` · ${r.controlsNeedsReview} review` : ''}
                  </td>
                  <td style={td}>{new Date(r.generatedAt).toLocaleString()}</td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>{r.attestationKeyId ?? '—'}</code>
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
export function TasksPage() {
  return <Placeholder title="Tugas" note="Kartu tugas shop-floor — segera." />;
}
function Sparkline({
  series,
  low,
  high,
}: {
  series: number[];
  low: number | null;
  high: number | null;
}) {
  const w = 240;
  const h = 48;
  if (series.length === 0) return <svg width={w} height={h} />;
  const min = Math.min(...series, low ?? Math.min(...series));
  const max = Math.max(...series, high ?? Math.max(...series));
  const span = max - min || 1;
  const pad = 4;
  const inner = h - pad * 2;
  const stepX = series.length > 1 ? w / (series.length - 1) : 0;
  const path = series
    .map((v, i) => {
      const x = i * stepX;
      const y = pad + inner - ((v - min) / span) * inner;
      return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join(' ');
  const bandY = (v: number) => pad + inner - ((v - min) / span) * inner;
  return (
    <svg width={w} height={h} role="img" aria-label="temperature sparkline">
      {low != null && high != null ? (
        <rect
          x={0}
          y={bandY(high)}
          width={w}
          height={Math.max(0, bandY(low) - bandY(high))}
          fill="var(--aether-accent)"
          opacity={0.08}
        />
      ) : null}
      <path d={path} stroke="var(--aether-accent)" strokeWidth={1.5} fill="none" />
    </svg>
  );
}

function summarize(readings: TemperatureReading[]) {
  const bySensor = new Map<string, TemperatureReading[]>();
  for (const r of readings) {
    const list = bySensor.get(r.sensorId) ?? [];
    list.push(r);
    bySensor.set(r.sensorId, list);
  }
  for (const list of bySensor.values()) {
    list.sort((a, b) => a.takenAt.localeCompare(b.takenAt));
  }
  return Array.from(bySensor.entries()).map(([sensorId, list]) => {
    const latest = list[list.length - 1]!;
    const series = list.map((r) => r.celsius);
    const outOfRange = list.filter(
      (r) => (r.lowC != null && r.celsius < r.lowC) || (r.highC != null && r.celsius > r.highC),
    ).length;
    const inRange =
      latest.lowC == null || latest.highC == null
        ? true
        : latest.celsius >= latest.lowC && latest.celsius <= latest.highC;
    return { sensorId, latest, series, outOfRange, inRange };
  });
}

export function ColdChainPage() {
  const { data: readings = [], isLoading, isError } = useColdChainReadings(240);
  const sensors = summarize(readings);

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Rantai Dingin</h2>
      <p style={muted}>
        Pemantauan suhu real-time per sensor (kapabilitas industri <code>cold-chain-monitor</code>).
        Auto-refresh 30 detik. Hijau = di dalam rentang, kuning = pernah keluar rentang dalam 1 jam
        terakhir, merah = sedang keluar rentang.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
          gap: 12,
        }}
      >
        {sensors.map((s) => {
          const kind: StatusKind = !s.inRange ? 'danger' : s.outOfRange > 0 ? 'warning' : 'success';
          return (
            <Card key={s.sensorId}>
              <CardHeader title={s.latest.sensorLabel || s.sensorId} subtitle={s.sensorId} />
              <CardBody>
                <Stack gap={8}>
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: 10 }}>
                    <span style={{ fontSize: 24, fontWeight: 600 }}>{s.latest.celsius}°C</span>
                    <StatusPill kind={kind}>
                      {!s.inRange
                        ? 'di luar rentang'
                        : s.outOfRange > 0
                          ? 'pernah drift'
                          : 'normal'}
                    </StatusPill>
                  </div>
                  <Sparkline series={s.series} low={s.latest.lowC} high={s.latest.highC} />
                  <p style={muted}>
                    Target {s.latest.lowC ?? '–'}°C…{s.latest.highC ?? '–'}°C · {s.outOfRange}{' '}
                    sampel di luar rentang · {new Date(s.latest.takenAt).toLocaleTimeString()}
                  </p>
                </Stack>
              </CardBody>
            </Card>
          );
        })}
      </div>
    </Stack>
  );
}
interface LotNode extends Lot {
  children: LotNode[];
}

function buildForest(lots: readonly Lot[]): LotNode[] {
  const byId = new Map<string, LotNode>();
  for (const l of lots) byId.set(l.id, { ...l, children: [] });
  const roots: LotNode[] = [];
  for (const node of byId.values()) {
    if (node.parentLotId && byId.has(node.parentLotId)) {
      byId.get(node.parentLotId)!.children.push(node);
    } else {
      roots.push(node);
    }
  }
  for (const node of byId.values()) {
    node.children.sort((a, b) => a.lotCode.localeCompare(b.lotCode));
  }
  roots.sort((a, b) => b.madeAt.localeCompare(a.madeAt));
  return roots;
}

function LotRow({ node, depth }: { node: LotNode; depth: number }) {
  const expires = node.expiresAt ? new Date(node.expiresAt) : null;
  const expiringSoon =
    expires != null &&
    expires.getTime() - Date.now() < 3 * 86_400_000 &&
    expires.getTime() > Date.now();
  const expired = expires != null && expires.getTime() <= Date.now();
  const kind: StatusKind = expired ? 'danger' : expiringSoon ? 'warning' : 'success';
  return (
    <>
      <tr style={{ borderTop: '1px solid var(--aether-border)' }}>
        <td style={td}>
          <span style={{ paddingLeft: depth * 18 }}>
            {depth > 0 ? <span style={{ color: 'var(--aether-fg-muted)' }}>↳ </span> : null}
            <code>{node.lotCode}</code>
          </span>
        </td>
        <td style={td}>{node.materialSku ?? '—'}</td>
        <td style={td}>
          {node.qty ?? '—'} {node.uom ?? ''}
        </td>
        <td style={td}>{new Date(node.madeAt).toLocaleDateString()}</td>
        <td style={td}>
          {expires ? (
            <StatusPill kind={kind}>
              {expired ? 'kedaluwarsa' : expiringSoon ? 'segera' : 'aman'} ·{' '}
              {expires.toLocaleDateString()}
            </StatusPill>
          ) : (
            '—'
          )}
        </td>
        <td style={td}>{node.description ?? '—'}</td>
      </tr>
      {node.children.map((child) => (
        <LotRow key={child.id} node={child} depth={depth + 1} />
      ))}
    </>
  );
}

export function TraceabilityPage() {
  const { data: lots = [], isLoading, isError } = useLots(200);
  const forest = buildForest(lots);

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Telusur Lot</h2>
      <p style={muted}>
        Silsilah lot/batch (kapabilitas industri <code>lot-genealogy</code>). Lot anak (turunan
        produksi) menjorok ke kanan dengan tanda ↳. Status kadaluwarsa: hijau = aman, kuning =
        kedaluwarsa &lt;3 hari, merah = sudah kedaluwarsa.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr>
                <th style={th}>Kode lot</th>
                <th style={th}>SKU</th>
                <th style={th}>Jumlah</th>
                <th style={th}>Dibuat</th>
                <th style={th}>Kedaluwarsa</th>
                <th style={th}>Keterangan</th>
              </tr>
            </thead>
            <tbody>
              {forest.map((root) => (
                <LotRow key={root.id} node={root} depth={0} />
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
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
