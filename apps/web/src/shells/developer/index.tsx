import { Routes, Route, NavLink, Navigate } from 'react-router-dom';
import { useTranslation } from '@aether/i18n';
import { InfrastructurePage } from './InfrastructurePage';
import { ModuleRegistryPage } from './ModuleRegistryPage';
import { AuditLogPage } from './AuditLogPage';
import { SystemHealthPage } from './SystemHealthPage';
import { DiscoveryPage } from './DiscoveryPage';
import { IndustryProfilePage } from './IndustryProfilePage';
import { ShellLayout } from '../_shared/ShellLayout';

const NAV = [
  { to: 'infrastructure', labelKey: 'nav.developer.infrastructure' },
  { to: 'discovery', labelKey: 'nav.developer.discovery' },
  { to: 'industry', labelKey: 'nav.developer.industry' },
  { to: 'modules', labelKey: 'nav.developer.modules' },
  { to: 'audit', labelKey: 'nav.developer.audit' },
  { to: 'health', labelKey: 'nav.developer.health' },
];

export default function DeveloperShell() {
  const { t } = useTranslation();
  return (
    <ShellLayout
      title={t('shell.developer.title')}
      subtitle={t('shell.developer.subtitle')}
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
