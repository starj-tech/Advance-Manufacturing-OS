import type { Role } from '@aether/rpc-contracts';

export interface ProductNavItem {
  id: string;
  label: string;
  /** Permission scope required to see this item (wildcard-aware). */
  permission?: string;
  /** Industry capability required to see this item. */
  capability?: string;
}

/**
 * Per-role navigation for the Main product. Items are further filtered at
 * render time by the user's permissions (hasPermission) and the tenant's
 * industry capabilities — so two managers in different verticals see
 * different tools.
 */
export const ROLE_NAV: Record<Role, ProductNavItem[]> = {
  executive: [
    { id: 'overview', label: 'Ringkasan' },
    { id: 'compliance', label: 'Kepatuhan' },
  ],
  manager: [
    { id: 'work-orders', label: 'Work Order', permission: 'work_orders:read' },
    { id: 'machines', label: 'Mesin', permission: 'machines:read' },
    { id: 'cold-chain', label: 'Rantai Dingin', capability: 'cold-chain-monitor' },
    { id: 'traceability', label: 'Telusur Lot', capability: 'lot-genealogy' },
  ],
  employee: [
    { id: 'tasks', label: 'Tugas', permission: 'tasks:read' },
    { id: 'machines', label: 'Mesin' },
  ],
  developer: [
    { id: 'machines', label: 'Mesin' },
    { id: 'work-orders', label: 'Work Order' },
  ],
  it: [
    { id: 'machines', label: 'Mesin' },
    { id: 'overview', label: 'Ringkasan' },
  ],
};
