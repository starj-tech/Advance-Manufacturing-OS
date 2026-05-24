import { NavLink } from 'react-router-dom';
import { gloveTokens } from '@aether/glove-kit';
import { useTranslation } from '@aether/i18n';

const ITEMS = [
  { to: 'tasks', labelKey: 'employee.nav.tasks' },
  { to: 'sos', labelKey: 'employee.nav.sos' },
  { to: 'clock', labelKey: 'employee.nav.clock' },
];

export function EmployeeNav() {
  const { t } = useTranslation();
  return (
    <nav
      style={{
        display: 'grid',
        gridTemplateColumns: `repeat(${ITEMS.length}, 1fr)`,
        borderTop: '1px solid var(--aether-border)',
        background: 'var(--aether-bg-elevated)',
      }}
    >
      {ITEMS.map((i) => (
        <NavLink
          key={i.to}
          to={i.to}
          style={({ isActive }) => ({
            minHeight: gloveTokens.touchTargetMin + 16,
            display: 'grid',
            placeItems: 'center',
            color: isActive ? 'var(--aether-fg)' : 'var(--aether-fg-muted)',
            fontSize: gloveTokens.fontSizePrimary,
            fontWeight: 600,
            background: isActive ? 'var(--aether-bg)' : 'transparent',
            borderTop: isActive ? '3px solid var(--aether-accent)' : '3px solid transparent',
          })}
        >
          {t(i.labelKey)}
        </NavLink>
      ))}
    </nav>
  );
}
