import { Routes, Route, NavLink, Navigate } from 'react-router-dom';
import { ShellLayout } from '../_shared/ShellLayout';
import { OverviewPage } from './OverviewPage';
import { DigitalTwinPage } from './DigitalTwinPage';
import { ProjectionsPage } from './ProjectionsPage';

const NAV = [
  { to: 'overview', label: 'Overview' },
  { to: 'digital-twin', label: 'Digital twin' },
  { to: 'projections', label: 'AI projections' },
];

export default function ExecutiveShell() {
  return (
    <ShellLayout
      title="Executive"
      subtitle="Vision · Strategy · Capital allocation"
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
        <Route index element={<Navigate to="overview" replace />} />
        <Route path="overview" element={<OverviewPage />} />
        <Route path="digital-twin" element={<DigitalTwinPage />} />
        <Route path="projections" element={<ProjectionsPage />} />
      </Routes>
    </ShellLayout>
  );
}
