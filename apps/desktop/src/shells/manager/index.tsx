import { Routes, Route, NavLink, Navigate } from 'react-router-dom';
import { useTranslation } from '@aether/i18n';
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
  { to: 'work-orders', labelKey: 'nav.manager.workOrders' },
  { to: 'machines', labelKey: 'nav.manager.machines' },
  { to: 'maintenance', labelKey: 'nav.manager.maintenance' },
  { to: 'inventory', labelKey: 'nav.manager.inventory' },
  { to: 'supply-chain', labelKey: 'nav.manager.supplyChain' },
  { to: 'roster', labelKey: 'nav.manager.roster' },
  { to: 'certifications', labelKey: 'nav.manager.certifications' },
  { to: 'support', labelKey: 'nav.manager.support' },
];

export default function ManagerShell() {
  const { t } = useTranslation();
  return (
    <ShellLayout
      title={t('shell.manager.title')}
      subtitle={t('shell.manager.subtitle')}
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
              {t(n.labelKey)}
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
