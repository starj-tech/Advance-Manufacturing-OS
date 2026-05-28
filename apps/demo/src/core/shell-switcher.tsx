import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { SHELLS, useShellStore, hasPermission } from '@aether/shell-runtime';
import type { Role } from '@aether/shell-runtime';
import { useSession } from './session-provider';

interface SwitchOptions {
  reason?: 'manual' | 'role-change' | 'auto';
}

export interface ShellSwitcher {
  switchShell: (target: Role, opts?: SwitchOptions) => void;
}

/**
 * Dynamic Shell switching mechanism.
 *
 * Performs hot-swap WITHOUT page reload to keep Supabase Realtime, IPC
 * channels, and in-flight commands alive across role transitions.
 *
 * Steps:
 *   1. Verify the user is allowed to assume the target shell.
 *   2. Persist a snapshot of the current shell's ephemeral state.
 *   3. Reset per-shell stores to avoid cross-role data bleed.
 *   4. Invalidate query cache scoped to the previous shell.
 *   5. React Router navigates to the lazy chunk; Suspense triggers load.
 *
 * Server-side RLS is the authoritative gate. The client-side check here
 * is purely UX (avoid rendering a shell the user cannot access).
 */
export function useShellSwitcher(): ShellSwitcher {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const activeShell = useShellStore((s) => s.activeShell);
  const saveSnapshot = useShellStore((s) => s.saveSnapshot);
  const { session } = useSession();

  const switchShell = useCallback(
    (target: Role, _opts: SwitchOptions = {}) => {
      if (!session) return;

      const targetShell = SHELLS.find((s) => s.id === target);
      if (!targetShell) {
        console.warn('[shell-switcher] unknown shell', target);
        return;
      }

      // 1. Authorization (UX gate; server-side RLS is authoritative).
      const scope = `shell:${target}`;
      const allowed =
        target === session.primaryRole ||
        hasPermission(session.permissions, scope) ||
        session.permissions.includes('*');
      if (!allowed) {
        console.warn('[shell-switcher] denied', target, session.permissions);
        return;
      }

      // 2. Persist a snapshot of where we were.
      if (activeShell) {
        saveSnapshot(activeShell, { at: Date.now(), pathname: window.location.pathname });
      }

      // 3. Drop ephemeral query data scoped to the previous shell so the
      //    new shell never sees stale data from a different role.
      if (activeShell) {
        queryClient.removeQueries({ queryKey: ['shell', activeShell] });
      }

      // 4. Navigate. Suspense + React.lazy handle chunk loading.
      navigate(targetShell.homePath);
    },
    [navigate, queryClient, activeShell, saveSnapshot, session],
  );

  return { switchShell };
}
