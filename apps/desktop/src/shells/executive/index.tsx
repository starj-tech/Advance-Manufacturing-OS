import { Routes, Route, NavLink, Navigate } from 'react-router-dom';
import { useTranslation } from '@aether/i18n';
import { ShellLayout } from '../_shared/ShellLayout';
import { OverviewPage } from './OverviewPage';
import { DigitalTwinPage } from './DigitalTwinPage';
import { ProjectionsPage } from './ProjectionsPage';
import { CompliancePage } from './CompliancePage';

const NAV = [
  { to: 'overview', labelKey: 'nav.executive.overview' },
  { to: 'digital-twin', labelKey: 'nav.executive.digitalTwin' },
  { to: 'projections', labelKey: 'nav.executive.projections' },
  { to: 'compliance', labelKey: 'nav.executive.compliance' },
];

export default function ExecutiveShell() {
  const { t } = useTranslation();
  return (
    <ShellLayout
      title={t('shell.executive.title')}
      subtitle={t('shell.executive.subtitle')}
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
        <Route index element={<Navigate to="overview" replace />} />
        <Route path="overview" element={<OverviewPage />} />
        <Route path="digital-twin" element={<DigitalTwinPage />} />
        <Route path="projections" element={<ProjectionsPage />} />
        <Route path="compliance" element={<CompliancePage />} />
      </Routes>
    </ShellLayout>
  );
}
