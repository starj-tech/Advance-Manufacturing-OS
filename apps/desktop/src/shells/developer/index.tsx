import { Routes, Route, NavLink, Navigate } from 'react-router-dom';
import { InfrastructurePage } from './InfrastructurePage';
import { ModuleRegistryPage } from './ModuleRegistryPage';
import { AuditLogPage } from './AuditLogPage';
import { SystemHealthPage } from './SystemHealthPage';
import { DiscoveryPage } from './DiscoveryPage';
import { IndustryProfilePage } from './IndustryProfilePage';
import { ShellLayout } from '../_shared/ShellLayout';

const NAV = [
  { to: 'infrastructure', label: 'Infrastructure' },
  { to: 'discovery', label: 'IoT discovery' },
  { to: 'industry', label: 'Industry profile' },
  { to: 'modules', label: 'Module registry' },
  { to: 'audit', label: 'Audit log' },
  { to: 'health', label: 'System health' },
];

export default function DeveloperShell() {
  return (
    <ShellLayout
      title="Developer"
      subtitle="Infrastructure · Modules · Audit · Telemetry"
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
        <Route index element={<Navigate to="infrastructure" replace />} />
        <Route path="infrastructure" element={<InfrastructurePage />} />
        <Route path="discovery" element={<DiscoveryPage />} />
        <Route path="industry" element={<IndustryProfilePage />} />
        <Route path="modules" element={<ModuleRegistryPage />} />
        <Route path="audit" element={<AuditLogPage />} />
        <Route path="health" element={<SystemHealthPage />} />
      </Routes>
    </ShellLayout>
  );
}
