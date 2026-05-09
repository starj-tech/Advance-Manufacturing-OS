import { createContext, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

/**
 * Industry-specific capability flags, surfaced from `aether-industry`'s
 * `IndustryProfile` to the React tree. Components call
 * `useCapability("expired-date-tracking")` to gate rendering on the
 * tenant's active industry profile.
 *
 * The hook is intentionally simple — boolean by capability id — so that
 * component code stays declarative:
 *
 *   const showExpiry = useCapability('expired-date-tracking');
 *   {showExpiry ? <ExpiryColumn /> : <SerialColumn />}
 *
 * Override semantics (PR #6 wires these to `tenant_capability_overrides`):
 *   - explicit `true`  → always visible regardless of profile default
 *   - explicit `false` → always hidden regardless of profile default
 *   - omitted          → fall back to the IndustryProfile default
 */
export interface CapabilityContextValue {
  /** Active industry slug, or null while the tenant is unprovisioned. */
  industrySlug: string | null;
  /** Capability ids granted by the active IndustryProfile. */
  granted: ReadonlySet<string>;
  /** Per-tenant force-on/off overrides. */
  overrides: ReadonlyMap<string, boolean>;
  setIndustry: (slug: string | null, granted: ReadonlySet<string>) => void;
  setOverride: (id: string, enabled: boolean | null) => void;
}

const CapabilityContext = createContext<CapabilityContextValue | null>(null);

export interface CapabilityProviderProps {
  /** Initial industry slug — typically resolved by an IPC call in App.tsx. */
  initialIndustrySlug?: string | null;
  /** Initial granted capability ids from the IndustryProfile resolver. */
  initialGranted?: ReadonlyArray<string>;
  /** Optional per-tenant overrides (id → enabled). */
  initialOverrides?: ReadonlyMap<string, boolean>;
  children: ReactNode;
}

export function CapabilityProvider({
  initialIndustrySlug = null,
  initialGranted = [],
  initialOverrides,
  children,
}: CapabilityProviderProps) {
  const [industrySlug, setSlug] = useState<string | null>(initialIndustrySlug);
  const [granted, setGranted] = useState<ReadonlySet<string>>(() => new Set(initialGranted));
  const [overrides, setOverrides] = useState<ReadonlyMap<string, boolean>>(
    () => initialOverrides ?? new Map(),
  );

  const value = useMemo<CapabilityContextValue>(
    () => ({
      industrySlug,
      granted,
      overrides,
      setIndustry: (slug, g) => {
        setSlug(slug);
        setGranted(g);
      },
      setOverride: (id, enabled) => {
        const next = new Map(overrides);
        if (enabled === null) {
          next.delete(id);
        } else {
          next.set(id, enabled);
        }
        setOverrides(next);
      },
    }),
    [industrySlug, granted, overrides],
  );

  return <CapabilityContext.Provider value={value}>{children}</CapabilityContext.Provider>;
}

function useCapabilityContext(): CapabilityContextValue {
  const ctx = useContext(CapabilityContext);
  if (!ctx) {
    throw new Error('useCapability must be used within <CapabilityProvider>');
  }
  return ctx;
}

/**
 * Hook returning whether `id` is enabled for the active tenant.
 *
 * Resolution order:
 *   1. explicit override (force-on or force-off)
 *   2. IndustryProfile default (membership in `granted`)
 */
export function useCapability(id: string): boolean {
  const { granted, overrides } = useCapabilityContext();
  const ov = overrides.get(id);
  if (ov !== undefined) return ov;
  return granted.has(id);
}

/** Hook returning the active industry slug (null = unprovisioned tenant). */
export function useIndustry(): string | null {
  return useCapabilityContext().industrySlug;
}

/**
 * Hook returning the setter that swaps the active industry profile +
 * its capability set. PR #6 wires this to the `industry_set` Tauri
 * command which persists to `tenant_industry`.
 */
export function useSetIndustry(): (
  slug: string | null,
  granted: ReadonlyArray<string>,
) => void {
  const ctx = useCapabilityContext();
  return (slug, granted) => ctx.setIndustry(slug, new Set(granted));
}

export interface IndustryGateProps {
  /** Capability id required to render `children`. */
  capability: string;
  /** Optional fallback when the capability is not granted. */
  fallback?: ReactNode;
  children: ReactNode;
}

/**
 * Conditional render based on a capability flag. Equivalent to
 * `{useCapability(id) ? children : fallback}` but reads better at the
 * call site.
 */
export function IndustryGate({ capability, fallback = null, children }: IndustryGateProps) {
  return useCapability(capability) ? <>{children}</> : <>{fallback}</>;
}
