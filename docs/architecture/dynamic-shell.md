# Dynamic Shell

One web bundle; four faces. The shell switches based on the user's
role without reloading the page — keeping Supabase Realtime
subscriptions and in-flight requests alive across role transitions.

## Shells

| Shell     | Persona               | Tone                         | Notes                                                 |
| --------- | --------------------- | ---------------------------- | ----------------------------------------------------- |
| Developer | Platform engineer     | Dense, technical             | Module registry, audit log, system health             |
| Executive | Leadership            | Strategic, visual            | Digital twin, KPI cards, AI projections               |
| Manager   | Operations supervisor | Operational, action-oriented | Work orders, machines, maintenance, inventory, roster |
| Employee  | Shop-floor operator   | Glove-friendly, safety-first | Tasks, SOS, clock in/out — uses `@aether/glove-kit`   |

## Switching mechanism

`apps/demo/src/core/shell-switcher.tsx`:

1. Verify the user is allowed to assume the target shell (UX gate; RLS
   is the authoritative gate server-side).
2. Persist a snapshot of the current shell's ephemeral state.
3. Drop query cache scoped to the previous shell to prevent
   cross-role data bleed.
4. Navigate to the target shell's `homePath`. React Suspense + lazy
   chunks load the bundle.

`apps/demo/src/chrome/ChromeLayout.tsx` provides the shared chrome
(top bar, notifications, command palette) that stays mounted across
shell switches.

## Bundle layout

`apps/demo/vite.config.ts` declares manualChunks per shell so:

- `shell-developer.js`, `shell-executive.js`, `shell-manager.js`,
  `shell-employee.js` are loaded only when their shell is active.
- `ui-kit.js` and `glove-kit.js` are split for cache stability.
- Initial chunk holds chrome + router + session provider only — budget
  enforced at < 350KB gzipped in CI.

## Permission gating

Two layers:

1. **Server (RLS + RPC)** — authoritative. `supabase/migrations/0003_rls.sql::has_permission`.
2. **Client (UX)** — `packages/shell-runtime/src/permission.tsx`. Reads
   from JWT custom claims (`app_metadata.permissions`) populated by the
   `mint-session` Edge Function.

Wildcard support: `*` (all), `work_orders:*` (all actions on resource).

## Login flow (skeleton)

PR #1 ships a dev-mode login that lets you pick any of the four roles
without authenticating. PR #2 replaces it with WebAuthn passkey ↔
Supabase Auth.
