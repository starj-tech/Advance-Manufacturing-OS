export type { Role } from '@aether/rpc-contracts';
export { SHELLS, isShellRoute } from './shells';
export { useShellStore, resetShellState } from './store';
export { usePermission, hasPermission, PermissionGate } from './permission';
export type { PermissionGateProps } from './permission';
export {
  CapabilityProvider,
  useCapability,
  useIndustry,
  useSetIndustry,
  IndustryGate,
} from './capability';
export type {
  CapabilityContextValue,
  CapabilityProviderProps,
  IndustryGateProps,
} from './capability';
