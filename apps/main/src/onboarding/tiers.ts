// The three commercial bundles. Display prices live here; the matching Stripe
// price IDs are configured per-environment and consumed by the create-checkout
// Edge Function (Phase C backend). Recurring subscriptions, monthly or annual.

export type TierSlug = 'standard-node' | 'advanced-automata' | 'global-enterprise';
export type BillingCycle = 'monthly' | 'annual';

export interface Tier {
  slug: TierSlug;
  name: string;
  audience: string;
  /** Indicative monthly price in USD (display only). */
  monthlyUsd: number;
  /** Indicative annual price in USD — ~2 months free vs monthly. */
  annualUsd: number;
  seats: string;
  features: readonly string[];
  highlighted: boolean;
}

export const TIERS: readonly Tier[] = [
  {
    slug: 'standard-node',
    name: 'Standard Node',
    audience: 'Pabrik skala menengah',
    monthlyUsd: 499,
    annualUsd: 4990,
    seats: 'Hingga 50 akun',
    features: [
      '4 shell peran (Executive/Manager/Employee/Developer)',
      'Aplikasi IT untuk tim IT',
      'Telemetri & work order real-time',
      'Kepatuhan ISO 9001 + 1 standar industri',
      'Dukungan email',
    ],
    highlighted: false,
  },
  {
    slug: 'advanced-automata',
    name: 'Advanced Automata',
    audience: 'Pabrik besar / multinasional',
    monthlyUsd: 1999,
    annualUsd: 19990,
    seats: 'Hingga 500 akun',
    features: [
      'Semua fitur Standard Node',
      'Multi-pabrik & multi-region',
      'Modul kustom + Module Injection',
      'Semua standar kepatuhan industri',
      'Auto-healing + analitik prediktif',
      'Dukungan prioritas',
    ],
    highlighted: true,
  },
  {
    slug: 'global-enterprise',
    name: 'Global Enterprise',
    audience: 'Korporasi global',
    monthlyUsd: 6999,
    annualUsd: 69990,
    seats: 'Akun tak terbatas',
    features: [
      'Semua fitur Advanced Automata',
      'Data residency (EU/US/Asia)',
      'SSO & SCIM, audit lanjutan',
      'SLA 99.9% + on-call khusus',
      'Onboarding & training terkelola',
    ],
    highlighted: false,
  },
];

export function tierBySlug(slug: TierSlug): Tier {
  const found = TIERS.find((t) => t.slug === slug);
  if (!found) throw new Error(`unknown tier: ${slug}`);
  return found;
}

/** Price shown for a tier + cycle, formatted as USD. */
export function priceLabel(tier: Tier, cycle: BillingCycle): string {
  const amount = cycle === 'monthly' ? tier.monthlyUsd : tier.annualUsd;
  const per = cycle === 'monthly' ? '/bulan' : '/tahun';
  return `$${amount.toLocaleString('en-US')}${per}`;
}
