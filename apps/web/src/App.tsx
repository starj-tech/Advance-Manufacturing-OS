import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from 'react-router-dom';
import { LocaleProvider } from '@aether/i18n';
import { CapabilityProvider } from '@aether/shell-runtime';
import { router } from './core/router';
import { SessionProvider } from './core/session-provider';
import { DEV_INDUSTRY_SLUG, DEV_INDUSTRY_CAPABILITIES } from './core/dev-industry';
import { useState } from 'react';

export function App() {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 30_000,
            gcTime: 5 * 60_000,
            retry: 1,
            refetchOnWindowFocus: false,
          },
        },
      }),
  );

  return (
    <QueryClientProvider client={queryClient}>
      <LocaleProvider>
        <CapabilityProvider
          initialIndustrySlug={DEV_INDUSTRY_SLUG}
          initialGranted={DEV_INDUSTRY_CAPABILITIES}
        >
          <SessionProvider>
            <RouterProvider router={router} />
          </SessionProvider>
        </CapabilityProvider>
      </LocaleProvider>
    </QueryClientProvider>
  );
}
