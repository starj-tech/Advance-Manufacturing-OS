import { Routes, Route, NavLink, Navigate } from 'react-router-dom';
import { ShellLayout } from '../_shared/ShellLayout';
import { WorkOrdersPage } from './WorkOrdersPage';
import { MachinesPage } from './MachinesPage';
import { MaintenancePage } from './MaintenancePage';
import { InventoryPage } from './InventoryPage';
import { RosterPage } from './RosterPage';
import { CertificationsPage } from './CertificationsPage';
import { SupplyChainPage } from './SupplyChainPage';
import { SupportPage } from './SupportPage';

const NAV = [
  { to: 'work-orders', label: 'Work orders' },
  { to: 'machines', label: 'Machines' },
  { to: 'maintenance', label: 'Maintenance' },
  { to: 'inventory', label: 'Inventory' },
  { to: 'supply-chain', label: 'Supply chain' },
  { to: 'roster', label: 'Roster' },
  { to: 'certifications', label: 'Certifications' },
  { to: 'support', label: 'AI support' },
];

export default function ManagerShell() {
  return (
    <ShellLayout
      title="Manager"
      subtitle="Operations · Maintenance · Inventory · People"
      sidebar={
        <nav style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          {NAV.map((n) => (
            <NavLink
              key={n.to}
              to={n.to}
              style={({ isActive }) => ({
                padding: '8px 12px',
                borderRadius: 6,
                fontSize: 13,
                color: isActive ? 'var(--aether-fg)' : 'var(--aether-fg-muted)',
                background: isActive ? 'var(--aether-bg-elevated)' : 'transparent',
              })}
            >
              {n.label}
            </NavLink>
          ))}
        </nav>
      }
    >
      <Routes>
        <Route index element={<Navigate to="work-orders" replace />} />
        <Route path="work-orders" element={<WorkOrdersPage />} />
        <Route path="machines" element={<MachinesPage />} />
        <Route path="maintenance" element={<MaintenancePage />} />
        <Route path="inventory" element={<InventoryPage />} />
        <Route path="supply-chain" element={<SupplyChainPage />} />
        <Route path="roster" element={<RosterPage />} />
        <Route path="certifications" element={<CertificationsPage />} />
        <Route path="support" element={<SupportPage />} />
      </Routes>
    </ShellLayout>
  );
}
