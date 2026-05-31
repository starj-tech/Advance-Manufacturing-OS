import type { CSSProperties } from 'react';
import { Button, Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import { useResetUserPassword, useTenantUsers } from '@aether/data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };
const th: CSSProperties = {
  padding: '10px 16px',
  fontSize: 12,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  textAlign: 'left',
  color: 'var(--aether-fg-muted)',
};
const td: CSSProperties = { padding: '10px 16px', fontSize: 13 };

export function UsersPage() {
  const { data: users = [], isLoading, isError } = useTenantUsers();
  const reset = useResetUserPassword();

  const onReset = async (userId: string, username: string) => {
    if (!window.confirm(`Reset kata sandi untuk ${username}?`)) return;
    const r = await reset.mutateAsync(userId);
    if (r.ok && r.password) {
      window.alert(
        `Kata sandi sementara untuk ${username}:\n\n${r.password}\n\n` +
          'Salin sekarang — pesan ini hanya muncul sekali. User wajib mengubahnya saat login.',
      );
    } else if (r.error) {
      window.alert(`Gagal: ${r.error}`);
    }
  };

  return (
    <Stack gap={16}>
      <Stack direction="row" justify="space-between" align="center">
        <h2 style={{ margin: 0, fontSize: 20 }}>Akun & Peran</h2>
        <Button variant="primary" size="sm" disabled>
          Impor CSV
        </Button>
      </Stack>
      <p style={muted}>
        Kelola akun karyawan: impor massal (CSV), reset kata sandi, dan ubah peran. Reset memanggil
        Edge Function <code>manage-users</code>.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Username</th>
                <th style={th}>Peran</th>
                <th style={th}>Sub-peran</th>
                <th style={th}>Status</th>
                <th style={th}></th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr key={u.userId} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <code>{u.username}</code>
                  </td>
                  <td style={td}>{u.role}</td>
                  <td style={td}>{u.subRole || '—'}</td>
                  <td style={td}>
                    {u.mustChangePassword ? (
                      <StatusPill kind="warning">Belum aktivasi</StatusPill>
                    ) : (
                      <StatusPill kind="success">Aktif</StatusPill>
                    )}
                  </td>
                  <td style={{ ...td, textAlign: 'right' }}>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={reset.isPending}
                      onClick={() => void onReset(u.userId, u.username)}
                    >
                      Reset sandi
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}
