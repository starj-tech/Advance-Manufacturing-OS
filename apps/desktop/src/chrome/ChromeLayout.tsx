import { Outlet, useLocation } from 'react-router-dom';
import { useEffect } from 'react';
import { useShellStore, isShellRoute } from '@aether/shell-runtime';
import { TopBar } from './TopBar';
import { NotificationCenter } from './NotificationCenter';
import { CommandPalette } from './CommandPalette';
import { useSession } from '../core/session-provider';

export function ChromeLayout() {
  const { session } = useSession();
  const location = useLocation();
  const setActiveShell = useShellStore((s) => s.setActiveShell);

  useEffect(() => {
    setActiveShell(isShellRoute(location.pathname));
  }, [location.pathname, setActiveShell]);

  if (!session) return null;

  return (
    <div
      style={{
        display: 'grid',
        gridTemplateRows: 'auto 1fr',
        height: '100%',
      }}
    >
      <TopBar />
      <main
        style={{
          overflow: 'auto',
          background: 'var(--aether-bg)',
          position: 'relative',
        }}
      >
        <Outlet />
        <NotificationCenter />
        <CommandPalette />
      </main>
    </div>
  );
}
