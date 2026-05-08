import type { Role } from '@aether/rpc-contracts';

export interface ShellDescriptor {
  id: Role;
  label: string;
  description: string;
  /** Default landing route inside the shell. */
  homePath: string;
  /** Whether this shell uses glove-kit instead of ui-kit. */
  gloveFriendly: boolean;
}

export const SHELLS: ReadonlyArray<ShellDescriptor> = [
  {
    id: 'developer',
    label: 'Developer',
    description: 'Infrastructure, modules, audit log',
    homePath: '/developer/infrastructure',
    gloveFriendly: false,
  },
  {
    id: 'executive',
    label: 'Executive',
    description: 'Digital twin, KPIs, AI projections',
    homePath: '/executive/overview',
    gloveFriendly: false,
  },
  {
    id: 'manager',
    label: 'Manager',
    description: 'Work orders, machines, maintenance, inventory',
    homePath: '/manager/work-orders',
    gloveFriendly: false,
  },
  {
    id: 'employee',
    label: 'Employee',
    description: 'Tasks, SOS, clock in/out',
    homePath: '/employee/tasks',
    gloveFriendly: true,
  },
];

export function isShellRoute(pathname: string): Role | null {
  const segment = pathname.split('/').filter(Boolean)[0];
  if (!segment) return null;
  const found = SHELLS.find((s) => s.id === segment);
  return found?.id ?? null;
}
