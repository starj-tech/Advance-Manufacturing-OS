# Universal Module — Industry-Specific Logic Injection

> Status: skeleton with full 21-industry catalog. Tenant-onboarding hook
>
> - per-tenant capability override land in PR #6.

The promise: a single binary that reshapes itself for the customer's
industry. Food plants get expiry tracking + cold chain monitoring;
automotive plants get parts serialization + precision calibration;
pharmaceutical plants get GxP electronic records + serialization. No
per-customer fork.

## Components

`crates/aether-industry`:

- `Industry` — 21 verticals (Food & Beverage, Pharmaceuticals,
  Automotive, Aerospace, Electronics, Chemical Process, Textile,
  Plastics, Metal Fabrication, Wood & Paper, Cement, Mining,
  Oil & Gas, Energy, Medical Devices, Cosmetics, Furniture, Glass &
  Ceramics, Rubber, Printing & Packaging, Battery & Renewables).
- `IndustryProfile` — `{ industry, capabilities, auto_modules,
default_standards }`.
- `Capability` — `{ id, kind, display, description }` where `kind ∈
{Tracking, Quality, Maintenance, Safety, Compliance, Process,
Sustainability}`. UI checks via `useCapability("...")`.
- `profile_for(industry)` — pure deterministic resolver. Reproducible,
  fully unit-tested.

## Data flow

```
Tenant onboarding picks Industry::FoodAndBeverage
                │
                ▼
   profile_for(FoodAndBeverage)
                │
   ┌────────────┼─────────────────┐
   │            │                 │
   ▼            ▼                 ▼
capabilities  auto_modules    default_standards
   │            │                 │
   │            │                 ▼
   │            │            seeds aether-compliance
   │            │            (tenant_compliance_scope)
   │            ▼
   │       installs `com.aether.cold-chain`,
   │       `com.aether.haccp` from registry
   ▼
shell-runtime exposes `useCapability("expired-date-tracking")`
```

## React surface

The `aether-industry` profile is bridged to React components via
`packages/shell-runtime/src/capability.tsx`:

```tsx
import { CapabilityProvider, useCapability, useIndustry, IndustryGate } from '@aether/shell-runtime';

// App.tsx wraps the router; initial profile is resolved via the
// `industry_profile` Tauri command (PR #6) or the dev-industry helper
// during the skeleton phase.
<CapabilityProvider initialIndustrySlug="food-and-beverage" initialGranted={[...]}>
  <RouterProvider router={router} />
</CapabilityProvider>

// In any component:
const showExpiry = useCapability('expired-date-tracking');
const industry = useIndustry();   // 'food-and-beverage' | 'automotive' | ...

// Or as a wrapper:
<IndustryGate capability="parts-serial-tracking" fallback={<ExpiryColumn />}>
  <SerialColumn />
</IndustryGate>
```

Override semantics — every capability resolves with this precedence:

1. Explicit `tenant_capability_overrides` (force-on or force-off)
2. `IndustryProfile.capabilities` membership (default)
3. Otherwise: not granted

The `Manager → Inventory` page in the desktop app uses this directly:
columns appear or disappear based on `useCapability` calls, with no
per-tenant fork in component code.

## Schema (`0007_industry.sql`)

- `tenant_industry` — single row per tenant; the assigned vertical.
- `tenant_capability_overrides` — per-tenant force-on / force-off of
  individual capabilities, with audit metadata.

The static catalog lives in code, not the DB. This is intentional: the
catalog ships with the binary so the resolver is deterministic, doesn't
require a network round-trip, and is replayable from any historical
audit log entry.

## Why static, not data-driven

A fully data-driven catalog (rows in Postgres) was considered and
rejected:

- Auditability — a customer asking "what capabilities did I have on
  2026-03-12?" can answer it from the binary version + the override
  table, without needing point-in-time queries against a config table.
- Test coverage — every profile is exercised by `profile_for(i)` unit
  tests. A DB-driven catalog requires fixtures and is harder to refactor.
- Versioning — adding/removing capabilities is a code change, which
  flows through the normal review pipeline.

## Verification

- `crates/aether-industry/src/profile.rs::tests` covers 4 invariants:
  every industry has ≥1 capability; food has expiry tracking; automotive
  has serial + calibration; pharma has GxP records.
- `industry.rs::tests::slugs_are_unique` prevents duplicate registrations.
- `industry_list` Tauri command returns all 21 verticals; the Developer
  Shell `IndustryProfilePage` renders a live preview.
