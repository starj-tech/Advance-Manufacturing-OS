import { NavLink } from 'react-router-dom';
import { gloveTokens } from '@aether/glove-kit';

const ITEMS = [
  { to: 'tasks', label: 'Tasks' },
  { to: 'sos', label: 'SOS' },
  { to: 'clock', label: 'Clock' },
];

export function EmployeeNav() {
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
          {i.label}
        </NavLink>
      ))}
    </nav>
  );
}
