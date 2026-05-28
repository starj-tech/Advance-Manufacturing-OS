/**
 * AETHER-OS industry catalog.
 *
 * The single source of truth for the 20+ manufacturing verticals the product
 * tailors itself to. A tenant's `tenant_industry.industry_slug` selects a
 * profile; `capabilities` drive the capability gating in @aether/shell-runtime
 * (useCapability/IndustryGate), and `complianceScope` seeds the tenant's
 * compliance standards. Apps resolve a slug → profile here instead of
 * hard-coding per-industry behavior.
 */

export interface IndustryProfile {
  /** Stable slug stored in `tenant_industry.industry_slug`. */
  slug: string;
  /** Human label (English base; translate in the UI layer). */
  label: string;
  /** Capability ids enabled for this vertical (consumed by useCapability). */
  capabilities: readonly string[];
  /** Compliance standard slugs this vertical is typically enrolled in. */
  complianceScope: readonly string[];
}

const ISO_9001 = 'iso-9001';

export const INDUSTRY_CATALOG: Readonly<Record<string, IndustryProfile>> = {
  'food-and-beverage': {
    slug: 'food-and-beverage',
    label: 'Food & Beverage',
    capabilities: [
      'expired-date-tracking',
      'cold-chain-monitor',
      'temperature-sensor-integration',
      'haccp-checks',
      'allergen-control',
      'recall-readiness',
      'lot-genealogy',
    ],
    complianceScope: [ISO_9001, 'iso-22000', 'fsma-204', 'haccp'],
  },
  pharmaceuticals: {
    slug: 'pharmaceuticals',
    label: 'Pharmaceuticals',
    capabilities: [
      'batch-records',
      'lot-genealogy',
      'cleanroom-monitoring',
      'temperature-sensor-integration',
      'recall-readiness',
      'electronic-signatures',
    ],
    complianceScope: [ISO_9001, 'gmp', 'fda-21-cfr-210', 'fda-21-cfr-211', 'fda-21-cfr-part-11'],
  },
  automotive: {
    slug: 'automotive',
    label: 'Automotive',
    capabilities: [
      'parts-serial-tracking',
      'vin-traceability',
      'torque-verification',
      'dimensional-inspection',
      'material-certification',
      'spc-monitoring',
    ],
    complianceScope: [ISO_9001, 'iatf-16949', 'iso-14001'],
  },
  aerospace: {
    slug: 'aerospace',
    label: 'Aerospace & Defense',
    capabilities: [
      'parts-serial-tracking',
      'material-certification',
      'dimensional-inspection',
      'weld-inspection',
      'first-article-inspection',
      'precision-calibration',
    ],
    complianceScope: [ISO_9001, 'as9100', 'nadcap', 'itar'],
  },
  electronics: {
    slug: 'electronics',
    label: 'Electronics & PCB Assembly',
    capabilities: [
      'parts-serial-tracking',
      'esd-protection',
      'solder-paste-inspection',
      'aoi-inspection',
      'lot-genealogy',
      'precision-calibration',
    ],
    complianceScope: [ISO_9001, 'ipc-a-610', 'rohs', 'reach'],
  },
  'medical-devices': {
    slug: 'medical-devices',
    label: 'Medical Devices',
    capabilities: [
      'parts-serial-tracking',
      'lot-genealogy',
      'cleanroom-monitoring',
      'electronic-signatures',
      'dimensional-inspection',
      'recall-readiness',
    ],
    complianceScope: [ISO_9001, 'iso-13485', 'fda-21-cfr-820', 'fda-21-cfr-part-11', 'mdr'],
  },
  chemicals: {
    slug: 'chemicals',
    label: 'Chemicals',
    capabilities: [
      'batch-records',
      'sds-management',
      'hazmat-handling',
      'emissions-monitoring',
      'material-certification',
      'lot-genealogy',
    ],
    complianceScope: [ISO_9001, 'iso-14001', 'reach', 'osha-psm'],
  },
  'plastics-rubber': {
    slug: 'plastics-rubber',
    label: 'Plastics & Rubber',
    capabilities: [
      'moisture-control',
      'temperature-sensor-integration',
      'dimensional-inspection',
      'yield-tracking',
      'material-certification',
    ],
    complianceScope: [ISO_9001, 'iso-14001'],
  },
  'metals-fabrication': {
    slug: 'metals-fabrication',
    label: 'Metals & Fabrication',
    capabilities: [
      'material-certification',
      'weld-inspection',
      'dimensional-inspection',
      'surface-finish-qc',
      'heat-treat-tracking',
    ],
    complianceScope: [ISO_9001, 'aws-d1-1'],
  },
  'textiles-apparel': {
    slug: 'textiles-apparel',
    label: 'Textiles & Apparel',
    capabilities: ['color-matching', 'yield-tracking', 'lot-genealogy', 'defect-mapping'],
    complianceScope: [ISO_9001, 'oeko-tex'],
  },
  'cosmetics-personal-care': {
    slug: 'cosmetics-personal-care',
    label: 'Cosmetics & Personal Care',
    capabilities: [
      'batch-records',
      'allergen-control',
      'expired-date-tracking',
      'lot-genealogy',
      'recall-readiness',
    ],
    complianceScope: [ISO_9001, 'iso-22716', 'gmp'],
  },
  semiconductors: {
    slug: 'semiconductors',
    label: 'Semiconductors',
    capabilities: [
      'cleanroom-monitoring',
      'esd-protection',
      'wafer-lot-tracking',
      'spc-monitoring',
      'precision-calibration',
      'yield-tracking',
    ],
    complianceScope: [ISO_9001, 'iso-14644', 'jedec'],
  },
  'industrial-machinery': {
    slug: 'industrial-machinery',
    label: 'Industrial Machinery',
    capabilities: [
      'parts-serial-tracking',
      'dimensional-inspection',
      'torque-verification',
      'assembly-traceability',
      'precision-calibration',
    ],
    complianceScope: [ISO_9001, 'ce-marking'],
  },
  'furniture-woodworking': {
    slug: 'furniture-woodworking',
    label: 'Furniture & Woodworking',
    capabilities: ['moisture-control', 'surface-finish-qc', 'yield-tracking', 'defect-mapping'],
    complianceScope: [ISO_9001, 'carb-atcm'],
  },
  packaging: {
    slug: 'packaging',
    label: 'Packaging',
    capabilities: ['color-matching', 'dimensional-inspection', 'yield-tracking', 'lot-genealogy'],
    complianceScope: [ISO_9001, 'iso-22000'],
  },
  'building-materials': {
    slug: 'building-materials',
    label: 'Building Materials',
    capabilities: [
      'material-certification',
      'dimensional-inspection',
      'moisture-control',
      'emissions-monitoring',
    ],
    complianceScope: [ISO_9001, 'ce-marking', 'astm'],
  },
  'paper-pulp': {
    slug: 'paper-pulp',
    label: 'Paper & Pulp',
    capabilities: [
      'moisture-control',
      'yield-tracking',
      'emissions-monitoring',
      'energy-monitoring',
    ],
    complianceScope: [ISO_9001, 'iso-14001', 'fsc'],
  },
  'glass-ceramics': {
    slug: 'glass-ceramics',
    label: 'Glass & Ceramics',
    capabilities: [
      'temperature-sensor-integration',
      'dimensional-inspection',
      'surface-finish-qc',
      'energy-monitoring',
    ],
    complianceScope: [ISO_9001, 'astm'],
  },
  'battery-energy-storage': {
    slug: 'battery-energy-storage',
    label: 'Battery & Energy Storage',
    capabilities: [
      'cell-lot-tracking',
      'temperature-sensor-integration',
      'spc-monitoring',
      'hazmat-handling',
      'recall-readiness',
    ],
    complianceScope: [ISO_9001, 'un-38-3', 'iec-62133'],
  },
  'oil-gas-equipment': {
    slug: 'oil-gas-equipment',
    label: 'Oil & Gas Equipment',
    capabilities: [
      'material-certification',
      'weld-inspection',
      'hazmat-handling',
      'dimensional-inspection',
      'asset-integrity-tracking',
    ],
    complianceScope: [ISO_9001, 'api-q1', 'osha-psm'],
  },
  'consumer-goods': {
    slug: 'consumer-goods',
    label: 'Consumer Goods',
    capabilities: ['lot-genealogy', 'expired-date-tracking', 'yield-tracking', 'recall-readiness'],
    complianceScope: [ISO_9001],
  },
  'agriculture-equipment': {
    slug: 'agriculture-equipment',
    label: 'Agriculture Equipment',
    capabilities: [
      'parts-serial-tracking',
      'torque-verification',
      'dimensional-inspection',
      'assembly-traceability',
    ],
    complianceScope: [ISO_9001, 'ce-marking'],
  },
};

/** All industry slugs, in catalog order. */
export function industrySlugs(): string[] {
  return Object.keys(INDUSTRY_CATALOG);
}

/** Resolve a profile by slug, or `undefined` if unknown. */
export function industryProfile(slug: string): IndustryProfile | undefined {
  return INDUSTRY_CATALOG[slug];
}

/** Capabilities for a slug (empty array if the slug is unknown). */
export function capabilitiesFor(slug: string): readonly string[] {
  return INDUSTRY_CATALOG[slug]?.capabilities ?? [];
}
