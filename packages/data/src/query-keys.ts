/** Tenant-scoped react-query keys, so the cache never leaks across tenants. */
export const keys = {
  machines: (tenantId: string) => ['machines', tenantId] as const,
  workOrders: (tenantId: string) => ['work-orders', tenantId] as const,
  materials: (tenantId: string) => ['materials', tenantId] as const,
  tenantIndustry: (tenantId: string) => ['tenant-industry', tenantId] as const,
};
