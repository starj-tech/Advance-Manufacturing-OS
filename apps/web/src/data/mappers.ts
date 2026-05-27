/**
 * Maps snake_case Supabase/PostgREST rows to the camelCase shapes defined by
 * @aether/rpc-contracts. Each mapper returns a plain object; the calling hook
 * validates it with the matching Zod schema, so this file owns only the
 * column-name translation (kept next to the `.select(...)` column list).
 *
 * Note: NUMERIC columns come back from PostgREST as strings — coerce with
 * Number() before the Zod `.number()` parse.
 */

export const MACHINE_COLUMNS = 'id,tenant_id,code,name,protocol_binding,status,last_heartbeat,hlc';

export function rowToMachine(r: Record<string, unknown>): Record<string, unknown> {
  return {
    id: r.id,
    tenantId: r.tenant_id,
    code: r.code,
    name: r.name,
    protocolBinding: r.protocol_binding ?? null,
    status: r.status,
    lastHeartbeat: r.last_heartbeat ?? null,
    hlc: r.hlc,
  };
}
