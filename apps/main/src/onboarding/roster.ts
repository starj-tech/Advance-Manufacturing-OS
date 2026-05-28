// Employee roster parsed from the onboarding CSV. The Stripe webhook turns
// each entry into a provisioned account (username + generated password).

export interface RosterEntry {
  name: string;
  email: string;
  role: string;
  subRole: string;
}

const VALID_ROLES = ['developer', 'executive', 'manager', 'employee', 'it'];

/**
 * Parse a roster CSV with columns: name,email,role,sub_role. Tolerant of a
 * missing or present header row and surrounding whitespace; blank lines are
 * skipped and an unknown role falls back to 'employee'.
 */
export function parseRosterCsv(text: string): RosterEntry[] {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (lines.length === 0) return [];

  const first = lines[0]!.toLowerCase();
  const hasHeader = first.startsWith('name') && first.includes('email');
  const rows = hasHeader ? lines.slice(1) : lines;

  const out: RosterEntry[] = [];
  for (const line of rows) {
    const cells = line.split(',').map((c) => c.trim());
    const name = cells[0] ?? '';
    const email = cells[1] ?? '';
    const role = cells[2] ?? 'employee';
    const subRole = cells[3] ?? '';
    if (!name && !email) continue;
    out.push({
      name,
      email,
      role: VALID_ROLES.includes(role) ? role : 'employee',
      subRole,
    });
  }
  return out;
}
