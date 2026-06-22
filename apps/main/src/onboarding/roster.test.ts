import { describe, it, expect } from 'vitest';
import { parseRosterCsv } from './roster';

describe('parseRosterCsv', () => {
  it('parses rows and skips the header', () => {
    const rows = parseRosterCsv(
      'name,email,role,sub_role\nAni,ani@acme.co,manager,Production\nBudi,budi@acme.co,employee,Operator',
    );
    expect(rows).toHaveLength(2);
    expect(rows[0]).toEqual({
      name: 'Ani',
      email: 'ani@acme.co',
      role: 'manager',
      subRole: 'Production',
    });
  });

  it('works without a header row and tolerates blank lines', () => {
    const rows = parseRosterCsv('\nCiti,citi@acme.co,executive\n\n');
    expect(rows).toHaveLength(1);
    expect(rows[0]?.role).toBe('executive');
    expect(rows[0]?.subRole).toBe('');
  });

  it('falls back to employee for an unknown role', () => {
    const rows = parseRosterCsv('Dedi,dedi@acme.co,wizard,');
    expect(rows[0]?.role).toBe('employee');
  });

  it('returns an empty array for empty input', () => {
    expect(parseRosterCsv('   \n  ')).toEqual([]);
  });
});
