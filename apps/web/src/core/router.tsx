import { createBrowserRouter, Navigate } from 'react-router-dom';
import { lazy, Suspense } from 'react';
import { ChromeLayout } from '../chrome/ChromeLayout';
import { LoginScreen } from './LoginScreen';
import { ProtectedRoute } from './ProtectedRoute';

// Lazy chunks per shell — keeps initial bundle small and matches the
// manualChunks split configured in vite.config.ts.
const DeveloperShell = lazy(() => import('../shells/developer'));
const ExecutiveShell = lazy(() => import('../shells/executive'));
const ManagerShell = lazy(() => import('../shells/manager'));
const EmployeeShell = lazy(() => import('../shells/employee'));

const ShellSuspense = ({ children }: { children: React.ReactNode }) => (
  <Suspense
    fallback={
      <div
        style={{
          display: 'grid',
          placeItems: 'center',
          height: '100%',
          color: 'var(--aether-fg-muted)',
        }}
      >
        Loading shell…
      </div>
    }
  >
    {children}
  </Suspense>
);

export const router = createBrowserRouter([
  {
    path: '/login',
    element: <LoginScreen />,
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <ChromeLayout />
      </ProtectedRoute>
    ),
    children: [
      { index: true, element: <Navigate to="/employee" replace /> },
      {
        path: 'developer/*',
        element: (
          <ShellSuspense>
            <DeveloperShell />
          </ShellSuspense>
        ),
      },
      {
        path: 'executive/*',
        element: (
          <ShellSuspense>
            <ExecutiveShell />
          </ShellSuspense>
        ),
      },
      {
        path: 'manager/*',
        element: (
          <ShellSuspense>
            <ManagerShell />
          </ShellSuspense>
        ),
      },
      {
        path: 'employee/*',
        element: (
          <ShellSuspense>
            <EmployeeShell />
          </ShellSuspense>
        ),
      },
    ],
  },
  {
    path: '*',
    element: <Navigate to="/" replace />,
  },
]);
