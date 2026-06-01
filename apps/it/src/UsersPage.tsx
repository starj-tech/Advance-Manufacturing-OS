import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { useImportRoster, useResetUserPassword, useTenantUsers } from '@aether/data';
import type { ImportRosterResult } from '@aether/data';

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
const textarea: CSSProperties = {
  width: '100%',
  minHeight: 160,
  padding: '10px 12px',
  fontSize: 12,
  fontFamily: 'monospace',
  background: 'var(--aether-bg)',
  color: 'var(--aether-fg)',
  border: '1px solid var(--aether-border)',
  borderRadius: 8,
  outline: 'none',
  resize: 'vertical',
};

const SAMPLE_CSV =
  'name,email,role,sub_role\n' +
  'Sari Lestari,sari@acme.co,manager,Quality\n' +
  'Budi Pratama,budi@acme.co,employee,Machine Operator\n';

function ImportPanel({ result, onClose }: { result: ImportRosterResult; onClose: () => void }) {
  return (
    <Card>
      <CardHeader title="Hasil impor" subtitle="Salin kredensial sebelum menutup" />
      <CardBody>
        <Stack gap={12}>
          <p style={muted}>
            {result.created.length} dibuat · {result.skipped.length} dilewati. Kata sandi sementara
            di bawah hanya muncul sekali. User wajib mengubahnya saat login pertama.
          </p>
          {result.created.length > 0 ? (
            <Card padded={false}>
              <CardBody>
                <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
                  <thead>
                    <tr>
                      <th style={th}>Username</th>
                      <th style={th}>Peran</th>
                      <th style={th}>Kata sandi</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.created.map((c) => (
                      <tr key={c.username} style={{ borderTop: '1px solid var(--aether-border)' }}>
                        <td style={td}>
                          <code>{c.username}</code>
                        </td>
                        <td style={td}>{c.role}</td>
                        <td style={td}>
                          <code>{c.password}</code>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </CardBody>
            </Card>
          ) : null}
          {result.skipped.length > 0 ? (
            <Card padded={false}>
              <CardBody>
                <p style={{ ...muted, marginBottom: 8 }}>Dilewati:</p>
                <ul style={{ margin: 0, paddingLeft: 18, fontSize: 12 }}>
                  {result.skipped.map((s) => (
                    <li key={s.username}>
                      <code>{s.username}</code> — {s.reason}
                    </li>
                  ))}
                </ul>
              </CardBody>
            </Card>
          ) : null}
          <div>
            <Button variant="primary" size="sm" onClick={onClose}>
              Tutup
            </Button>
          </div>
        </Stack>
      </CardBody>
    </Card>
  );
}

function ImportDialog({ onClose }: { onClose: () => void }) {
  const [csv, setCsv] = useState(SAMPLE_CSV);
  const [result, setResult] = useState<ImportRosterResult | null>(null);
  const importRoster = useImportRoster();

  const submit = async () => {
    const r = await importRoster.mutateAsync(csv);
    setResult(r);
  };

  if (result) return <ImportPanel result={result} onClose={onClose} />;

  return (
    <Card>
      <CardHeader title="Impor karyawan dari CSV" subtitle="Kolom: name, email, role, sub_role" />
      <CardBody>
        <Stack gap={12}>
          <p style={muted}>
            Tempel CSV (baris pertama header opsional). Setiap karyawan akan dibuat dengan kata
            sandi acak; user wajib mengubahnya saat login pertama. Username diturunkan dari
            local-part email.
          </p>
          <textarea style={textarea} value={csv} onChange={(e) => setCsv(e.target.value)} />
          {importRoster.isError ? (
            <StatusPill kind="danger">Gagal: {(importRoster.error as Error).message}</StatusPill>
          ) : null}
          <Stack direction="row" gap={8}>
            <Button
              variant="primary"
              size="sm"
              disabled={importRoster.isPending || csv.trim().length === 0}
              onClick={() => void submit()}
            >
              {importRoster.isPending ? 'Memproses…' : 'Impor'}
            </Button>
            <Button variant="ghost" size="sm" onClick={onClose}>
              Batal
            </Button>
          </Stack>
        </Stack>
      </CardBody>
    </Card>
  );
}

export function UsersPage() {
  const { data: users = [], isLoading, isError } = useTenantUsers();
  const reset = useResetUserPassword();
  const [importing, setImporting] = useState(false);

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
        <Button variant="primary" size="sm" onClick={() => setImporting(true)}>
          Impor CSV
        </Button>
      </Stack>
      <p style={muted}>
        Kelola akun karyawan: impor massal (CSV), reset kata sandi, dan ubah peran. Reset dan impor
        memanggil Edge Function <code>manage-users</code>; setiap tindakan ter-audit.
      </p>
      {importing ? <ImportDialog onClose={() => setImporting(false)} /> : null}
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
