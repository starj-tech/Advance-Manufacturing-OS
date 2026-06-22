# Instant Compliance Attestation

> Status: skeleton — 22-standard catalog + control-point taxonomy + report
> shape. Live probes + cloud signing land in PR #6.

## The pitch

Most factories pay external auditors and consultants tens of thousands
of dollars per year to gather evidence and attest to ISO / GDPR / FDA /
BPOM / SNI compliance. AETHER-OS replaces 80% of that work with
continuous, automatic evidence collection from the audit log,
certifications, interlock events, sync metadata, and crypto layer —
then issues a cloud-signed attestation report any auditor can verify
against the platform's public key.

## Components

`crates/aether-compliance`:

- `ComplianceStandard` — descriptor for one standard:
  `{ slug, display, jurisdiction, kind, summary, control_point_ids }`.
- `StandardKind` — `International | Industry | Regulation | National`.
- `CATALOG` — 22 standards shipped at launch (see below).
- `ControlPoint` — `{ id, display, description }`.
- `CONTROL_POINTS` — taxonomy of evidence-gatherable controls
  (currently 50+; new control_points are added as probes are written).
- `Probe` trait — `evaluate()` returns `(Verdict, evidence_string)`.
  Probes are pure with respect to time so reports are reproducible.
- `Verdict` — `Pass | Fail | NotApplicable | NeedsReview`.
- `ComplianceReport` — `{ id, tenant_id, standard_slug, generated_at,
status, controls, attestation_signature }`.

## Standards in the launch catalog (22)

International / industry:

- ISO 9001 (QMS), ISO 14001 (env), ISO 27001 (infosec),
  ISO 22000 (food), ISO 45001 (OHS), ISO 13485 (medical devices),
  ISO 50001 (energy), ISO 22716 (cosmetics GMP)
- IATF 16949 (automotive), AS9100D (aerospace)
- HACCP, FSSC 22000 (food)
- FSC (chain of custody)

Regulations:

- GDPR (EU), HIPAA (US), FDA 21 CFR Part 11 (electronic records),
  FDA 21 CFR Part 820 (US medical devices), CE / EU MDR (medical),
  RoHS (EU), REACH (EU), EU Battery Regulation (2023/1542)

National:

- BPOM (Indonesia), SNI (Indonesia)

## Schema (`0008_compliance.sql`)

- `tenant_compliance_scope` — which standards a tenant has enrolled in.
- `compliance_reports` — append-only ledger of generated reports with
  rolled-up status counters + canonical JSONB `controls` array +
  cloud-issued `attestation_signature`. RLS-isolated, no UPDATE/DELETE.
- `compliance_evidence` — hash chain so probes can prove their
  evidence existed at report time without exposing the underlying row.

## Cloud attestation flow

```
Client                                   Edge Function (compliance-attest)
──────                                   ─────────────────────────────────
For each enrolled standard:
  resolve probes from CATALOG
  evaluate() each → ControlVerdict[]
  ComplianceReport::from_verdicts()
  POST { reportId, tenantId, standardSlug,
         status, controlsHash }      ───►  re-fetch controls from DB
                                          re-hash, compare with payload
                                          sign canonical(report_payload)
                                          with platform Ed25519 key
                                          (key_id = "aether-attest-2026")
  ◄──── signature
  UPDATE compliance_reports
    SET attestation_signature = ?,
        attestation_key_id = ?
```

External auditors verify by:

1. Fetching the report JSON + signature from the customer's Supabase.
2. Verifying the Ed25519 signature against the platform's published
   public key (URL: `https://attest.aether-os.com/keys`).
3. Optionally cross-checking individual evidence hashes against
   `compliance_evidence`.

## Why catalog-in-code

Same rationale as `aether-industry`: deterministic resolution,
reproducible reports, easier diffing in PR review. Probes themselves
are dynamic dispatch trait objects so adding a probe doesn't require a
catalog migration.

## Verification

- `catalog.rs::tests` covers slug uniqueness and the "≥21 standards"
  marketing claim with a hard-floor assertion.
- `control.rs::tests::every_standard_references_known_control_points`
  catches drift between standards and the control taxonomy.
- `report.rs::tests` covers all 4 verdict aggregation paths
  (compliant / needs-review / non-compliant / fail-outranks-review).
- The Executive Shell `CompliancePage` renders a live status grid;
  `compliance_standards` IPC command returns the catalog for the picker.
