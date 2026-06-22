import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export type ComplianceStatus = 'compliant' | 'needs-review' | 'non-compliant';

export interface ComplianceReport {
  id: string;
  standardSlug: string;
  generatedAt: string;
  status: ComplianceStatus;
  controlsTotal: number;
  controlsPassed: number;
  controlsFailed: number;
  controlsNeedsReview: number;
  attestationKeyId: string | null;
}

const DEMO_REPORTS: ComplianceReport[] = [
  {
    id: 'r1',
    standardSlug: 'haccp',
    generatedAt: new Date(Date.now() - 12 * 3_600_000).toISOString(),
    status: 'compliant',
    controlsTotal: 18,
    controlsPassed: 17,
    controlsFailed: 0,
    controlsNeedsReview: 1,
    attestationKeyId: 'edge:v1',
  },
  {
    id: 'r2',
    standardSlug: 'iso-22000',
    generatedAt: new Date(Date.now() - 36 * 3_600_000).toISOString(),
    status: 'needs-review',
    controlsTotal: 24,
    controlsPassed: 21,
    controlsFailed: 0,
    controlsNeedsReview: 3,
    attestationKeyId: 'edge:v1',
  },
];

function rowToReport(r: Record<string, unknown>): ComplianceReport {
  return {
    id: String(r.id ?? ''),
    standardSlug: String(r.standard_slug ?? ''),
    generatedAt: String(r.generated_at ?? new Date().toISOString()),
    status: (r.status as ComplianceStatus) ?? 'needs-review',
    controlsTotal: Number(r.controls_total ?? 0),
    controlsPassed: Number(r.controls_passed ?? 0),
    controlsFailed: Number(r.controls_failed ?? 0),
    controlsNeedsReview: Number(r.controls_needs_review ?? 0),
    attestationKeyId: r.attestation_key_id == null ? null : String(r.attestation_key_id),
  };
}

/** Recent compliance reports for the active tenant; demo set when no backend. */
export function useComplianceReports(limit = 20) {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: ['compliance-reports', tenantId, limit],
    queryFn: async (): Promise<ComplianceReport[]> => {
      if (!supabase) return DEMO_REPORTS;
      const { data, error } = await supabase
        .from('compliance_reports')
        .select(
          'id,standard_slug,generated_at,status,controls_total,controls_passed,controls_failed,controls_needs_review,attestation_key_id',
        )
        .order('generated_at', { ascending: false })
        .limit(limit);
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToReport);
    },
    enabled: !supabase || tenantId.length > 0,
  });
}
