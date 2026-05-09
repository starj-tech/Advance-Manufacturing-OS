import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useCapability, useIndustry } from '@aether/shell-runtime';

interface BaseRow {
  sku: string;
  name: string;
  uom: string;
  qtyOnHand: number;
  qtyReserved: number;
}

interface FoodRow extends BaseRow {
  lot: string;
  expiresAt: string;
  storageTempC: number;
  storageTargetC: number;
}

interface AutomotiveRow extends BaseRow {
  partSerial: string;
  vinLink: string;
  lastCalibrated: string;
}

const FOOD_ROWS: FoodRow[] = [
  { sku: 'MILK-3L',  name: 'Whole milk 3L',     uom: 'btl', qtyOnHand: 4280, qtyReserved: 320, lot: 'L-26045-A', expiresAt: '2026-05-21', storageTempC: 2.6, storageTargetC: 4.0 },
  { sku: 'CHKN-WB',  name: 'Boneless chicken',  uom: 'kg',  qtyOnHand: 612,  qtyReserved: 80,  lot: 'L-26049-B', expiresAt: '2026-05-12', storageTempC: -0.5, storageTargetC: 0.0 },
  { sku: 'BUN-WHT',  name: 'White bun pack 12', uom: 'pkg', qtyOnHand: 980,  qtyReserved: 0,   lot: 'L-26050-A', expiresAt: '2026-05-15', storageTempC: 21.0, storageTargetC: 22.0 },
  { sku: 'YGT-BLU',  name: 'Yoghurt blueberry', uom: 'cup', qtyOnHand: 320,  qtyReserved: 0,   lot: 'L-26041-D', expiresAt: '2026-05-09', storageTempC: 5.4, storageTargetC: 4.0 },
];

const AUTO_ROWS: AutomotiveRow[] = [
  { sku: 'STC-3KW',  name: 'Stator core 3kW',   uom: 'pcs', qtyOnHand: 320, qtyReserved: 50, partSerial: 'AS-2026-04-091823', vinLink: 'WBADX-2026-…',  lastCalibrated: '2026-05-04' },
  { sku: 'RTR-12MM', name: 'Rotor shaft 12mm',  uom: 'pcs', qtyOnHand: 580, qtyReserved: 0,  partSerial: 'AS-2026-04-091824', vinLink: 'WBADX-2026-…',  lastCalibrated: '2026-04-22' },
  { sku: 'BRK-PD-F', name: 'Front brake pads',  uom: 'set', qtyOnHand: 410, qtyReserved: 0,  partSerial: 'AS-2026-04-091825', vinLink: '—',           lastCalibrated: '2026-04-30' },
];

function expiryUrgency(iso: string): 'success' | 'warning' | 'danger' {
  const days = (new Date(iso).getTime() - Date.now()) / 86_400_000;
  if (days < 3) return 'danger';
  if (days < 7) return 'warning';
  return 'success';
}

function tempUrgency(actual: number, target: number): 'success' | 'warning' | 'danger' {
  const delta = Math.abs(actual - target);
  if (delta > 2.0) return 'danger';
  if (delta > 1.0) return 'warning';
  return 'success';
}

export function InventoryPage() {
  const industry = useIndustry();
  const showExpiry = useCapability('expired-date-tracking');
  const showTemperature = useCapability('temperature-sensor-integration');
  const showSerial = useCapability('parts-serial-tracking');
  const showCalibration = useCapability('precision-calibration');

  const usingFoodView = showExpiry || showTemperature;
  const usingAutoView = showSerial || showCalibration;

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>Inventory</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          Live qty_on_hand. Adjustments flow through server RPC{' '}
          <code>inventory_adjust(delta)</code> to keep stock ≥ 0 atomic. Columns adapt to your{' '}
          <strong>industry profile</strong>{industry ? ` (${industry})` : ''} — Universal Module
          in action.
        </p>
      </header>

      {usingFoodView ? (
        <Card padded={false}>
          <CardHeader
            title="Lots & cold chain"
            subtitle="Food & Beverage view — FEFO ordering, temperature alarms"
          />
          <CardBody>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                  <th style={th}>SKU</th>
                  <th style={th}>Name</th>
                  <th style={th}>Lot</th>
                  {showExpiry ? <th style={th}>Expires</th> : null}
                  {showTemperature ? <th style={th}>Temp (°C)</th> : null}
                  <th style={th}>On hand</th>
                </tr>
              </thead>
              <tbody>
                {FOOD_ROWS.map((r) => (
                  <tr key={r.sku} style={{ borderTop: '1px solid var(--aether-border)' }}>
                    <td style={td}>
                      <code>{r.sku}</code>
                    </td>
                    <td style={td}>{r.name}</td>
                    <td style={td}>
                      <code>{r.lot}</code>
                    </td>
                    {showExpiry ? (
                      <td style={td}>
                        <StatusPill kind={expiryUrgency(r.expiresAt)}>{r.expiresAt}</StatusPill>
                      </td>
                    ) : null}
                    {showTemperature ? (
                      <td style={td}>
                        <StatusPill kind={tempUrgency(r.storageTempC, r.storageTargetC)}>
                          {r.storageTempC.toFixed(1)} / target {r.storageTargetC.toFixed(1)}
                        </StatusPill>
                      </td>
                    ) : null}
                    <td style={td}>
                      {r.qtyOnHand} {r.uom} ({r.qtyReserved} reserved)
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </CardBody>
        </Card>
      ) : null}

      {usingAutoView ? (
        <Card padded={false}>
          <CardHeader
            title="Parts & calibration"
            subtitle="Automotive view — VIN linkage, calibration cadence"
          />
          <CardBody>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                  <th style={th}>SKU</th>
                  <th style={th}>Name</th>
                  {showSerial ? <th style={th}>Serial</th> : null}
                  {showSerial ? <th style={th}>VIN</th> : null}
                  {showCalibration ? <th style={th}>Last cal.</th> : null}
                  <th style={th}>On hand</th>
                </tr>
              </thead>
              <tbody>
                {AUTO_ROWS.map((r) => (
                  <tr key={r.sku} style={{ borderTop: '1px solid var(--aether-border)' }}>
                    <td style={td}>
                      <code>{r.sku}</code>
                    </td>
                    <td style={td}>{r.name}</td>
                    {showSerial ? (
                      <td style={td}>
                        <code>{r.partSerial}</code>
                      </td>
                    ) : null}
                    {showSerial ? <td style={td}>{r.vinLink}</td> : null}
                    {showCalibration ? <td style={td}>{r.lastCalibrated}</td> : null}
                    <td style={td}>
                      {r.qtyOnHand} {r.uom} ({r.qtyReserved} reserved)
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </CardBody>
        </Card>
      ) : null}

      {!usingFoodView && !usingAutoView ? (
        <Card>
          <CardBody>
            <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              Generic inventory view. Set an Industry profile in Developer → Industry profile to
              see expiry tracking, cold-chain temperature, parts serialization, or calibration
              columns appear automatically.
            </p>
          </CardBody>
        </Card>
      ) : null}
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
