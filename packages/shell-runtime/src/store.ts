import { create } from 'zustand';
import type { Role } from '@aether/rpc-contracts';

interface ShellState {
  activeShell: Role | null;
  /** Snapshot of per-shell ephemeral state, keyed by role.
   *  In future PRs this is populated by ShellSwitcher.persistShellSnapshot(). */
  snapshots: Partial<Record<Role, unknown>>;
  notifications: number;
  setActiveShell: (shell: Role | null) => void;
  setNotifications: (n: number) => void;
  saveSnapshot: (shell: Role, snapshot: unknown) => void;
  loadSnapshot: (shell: Role) => unknown;
  reset: () => void;
}

export const useShellStore = create<ShellState>((set, get) => ({
  activeShell: null,
  snapshots: {},
  notifications: 0,
  setActiveShell: (shell) => set({ activeShell: shell }),
  setNotifications: (n) => set({ notifications: n }),
  saveSnapshot: (shell, snapshot) =>
    set({ snapshots: { ...get().snapshots, [shell]: snapshot } }),
  loadSnapshot: (shell) => get().snapshots[shell],
  reset: () => set({ activeShell: null, snapshots: {}, notifications: 0 }),
}));

/**
 * Called when the user switches shells or signs out.
 * Drops ephemeral per-shell state to avoid cross-role data bleed.
 *
 * Note: persisted state (e.g. cached query data, IPC subscriptions
 * that span shells) lives elsewhere and is NOT touched here.
 */
export function resetShellState() {
  useShellStore.getState().reset();
}
