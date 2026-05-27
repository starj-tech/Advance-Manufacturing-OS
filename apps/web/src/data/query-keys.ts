/**
 * Central, tenant-scoped react-query keys. Scoping every key by tenantId
 * keeps the cache from leaking across tenants on shell/session switches.
 */
export const keys = {
  machines: (tenantId: string) => ['machines', tenantId] as const,
  workOrders: (tenantId: string) => ['work-orders', tenantId] as const,
  materials: (tenantId: string) => ['materials', tenantId] as const,
};
