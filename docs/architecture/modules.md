# Module Injection

> Status: skeleton. Manifest parser + signature verifier are implemented;
> loader runtime ships in PR #4.

## Goals

- Add or update factory features without redeploying the binary.
- Tenant admins control which publishers they trust.
- Compromised publisher key cannot silently push code.

## Manifest (TOML)

`module.toml`:

```toml
[module]
id       = "com.aether.qc-spc"
name     = "Statistical Process Control"
version  = "1.4.2"
shells   = ["manager", "executive"]
min_aether = "0.3.0"

[entry]
ui      = "ui/index.js"        # ESM bundle, default = MountFn
backend = "backend.wasm"       # optional, WASI

[permissions]
read    = ["telemetry", "work_orders"]
write   = ["spc_charts"]
network = ["https://qc-vendor.example.com"]

[dependencies]
"@aether/ui-kit" = "^1.0.0"

[signature]
algorithm     = "ed25519"
public_key_id = "vendor-acme-2026"
```

Parsed by `crates/aether-modules/src/manifest.rs::Manifest::from_toml`.

## Signing & verification

- **Sign:** `aether-modsign sign --key vendor.key --manifest module.toml --bundle bundle.tar.gz --out signature.bin`
- **Bytes signed:** `Sha256(bundle_bytes) || manifest_bytes`
- **Verify:** `crates/aether-modules/src/verify.rs::verify_bundle` checks
  the publisher key id is in the tenant's
  `tenant_trusted_publishers` table.

## Sandbox

- **UI** runs in a Web Worker. Capability-gated host API
  (`packages/module-sdk/src/host-api.ts`). Calls outside `permissions`
  are rejected at the IPC boundary.
- **Backend** (optional) runs as WASI under wasmtime. PR #4 adds the
  capability bindings (`aether.read('telemetry')`, etc.).
- iframes / ShadowRealm rejected — see `docs/architecture/README.md`.

## Lifecycle

```
publish ─► registry (Supabase) ─► clients fetch ─► verify ─► cache to disk
                                                         │
                                                         └─► spawn worker
                                                            │
unmount ◄─ host kill-switch (status='disabled' broadcast)  │
                                                            ▼
                                                     mount in shell slot
```

## Versioning

- Semver. `tenant_modules.policy ∈ {auto, manual, pinned}`.
- Rollback by updating `tenant_modules.pinned_version`; clients re-pull.
- Real-time kill-switch via Supabase Realtime broadcast on
  `modules.status = 'disabled'`. Target unmount latency < 2s.

## Supply chain

- **Per-tenant trusted publisher list.** A tenant explicitly enrolls
  each publisher key. The platform does not maintain a global trust root.
- **Transparency log** (PR #4) — append-only `module_publish_log` table
  recording every published version.
