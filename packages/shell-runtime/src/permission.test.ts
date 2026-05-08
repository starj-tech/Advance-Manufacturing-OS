import { describe, expect, it } from 'vitest';
import { hasPermission } from './permission';

describe('hasPermission', () => {
  it('allows when star wildcard is granted', () => {
    expect(hasPermission(['*'], 'work_orders:approve')).toBe(true);
  });

  it('allows exact scope match', () => {
    expect(hasPermission(['work_orders:approve'], 'work_orders:approve')).toBe(true);
  });

  it('allows resource wildcard match', () => {
    expect(hasPermission(['work_orders:*'], 'work_orders:approve')).toBe(true);
    expect(hasPermission(['work_orders:*'], 'work_orders:create')).toBe(true);
  });

  it('denies when scope is not in the set', () => {
    expect(hasPermission(['work_orders:read'], 'work_orders:approve')).toBe(false);
  });

  it('does not let `work_orders:*` leak into other resources', () => {
    expect(hasPermission(['work_orders:*'], 'inventory:write')).toBe(false);
  });

  it('handles empty grants', () => {
    expect(hasPermission([], 'anything:read')).toBe(false);
  });
});
