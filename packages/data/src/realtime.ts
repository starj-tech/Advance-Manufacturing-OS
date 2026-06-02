/**
 * Realtime helpers: subscribe to Postgres CDC and invalidate react-query
 * caches so polling-based hooks immediately reflect new rows.
 *
 * Single useEffect-shaped subscription per (table, tenantId). When a row
 * arrives we invalidate the related queries rather than splicing into the
 * cache directly — that keeps the hook readers identical to the polling
 * path and avoids race conditions with ordering / pagination.
 */
import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

interface SubscribeOpts {
  /** Postgres table name (public schema). */
  table: string;
  /** react-query keys to invalidate on every CDC event. */
  invalidate: ReadonlyArray<readonly unknown[]>;
  /** Optional row-level filter, e.g. `tenant_id=eq.<uuid>`. */
  filter?: string;
}

/**
 * Subscribe to INSERT/UPDATE/DELETE on a Postgres table for the active
 * tenant. Triggers react-query invalidation on every event. No-op in demo
 * mode (no backend).
 */
export function useRealtimeInvalidate({ table, invalidate, filter }: SubscribeOpts): void {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  useEffect(() => {
    const client = supabase;
    if (!client || tenantId.length === 0) return;
    const channelName = `aether:${table}:${tenantId}`;
    const channel = client
      .channel(channelName)
      .on(
        'postgres_changes' as never,
        {
          event: '*',
          schema: 'public',
          table,
          ...(filter ? { filter } : {}),
        } as never,
        () => {
          for (const key of invalidate) {
            void qc.invalidateQueries({ queryKey: key });
          }
        },
      )
      .subscribe();
    return () => {
      void client.removeChannel(channel);
    };
  }, [table, filter, tenantId, qc, invalidate]);
}
