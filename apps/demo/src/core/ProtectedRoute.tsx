import { Navigate } from 'react-router-dom';
import type { ReactNode } from 'react';
import { useSession } from './session-provider';

export function ProtectedRoute({ children }: { children: ReactNode }) {
  const { session, loading } = useSession();

  if (loading) {
    return (
      <div
        style={{
          display: 'grid',
          placeItems: 'center',
          height: '100%',
          color: 'var(--aether-fg-muted)',
        }}
      >
        Initializing AETHER-OS…
      </div>
    );
  }

  if (!session) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}
