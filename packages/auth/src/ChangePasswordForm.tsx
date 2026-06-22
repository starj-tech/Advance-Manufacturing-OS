import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { completePasswordChange } from './sign-in';
import { useSession } from './session';

const field: CSSProperties = {
  width: '100%',
  padding: '10px 12px',
  fontSize: 14,
  background: 'var(--aether-bg)',
  color: 'var(--aether-fg)',
  border: '1px solid var(--aether-border)',
  borderRadius: 8,
  outline: 'none',
};
const labelStyle: CSSProperties = {
  display: 'block',
  fontSize: 12,
  color: 'var(--aether-fg-muted)',
  marginBottom: 6,
};
const muted: CSSProperties = { margin: 0, fontSize: 13, color: 'var(--aether-fg-muted)' };

/**
 * Forced first-login reset screen. Shown when session.mustChangePassword is
 * true; calls complete-password-change, then refreshes the session so the
 * cleared flag propagates and the normal product chrome renders.
 */
export function ChangePasswordForm() {
  const { session } = useSession();
  const [pw, setPw] = useState('');
  const [pw2, setPw2] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const submit = async () => {
    if (pw !== pw2) {
      setError('Konfirmasi tidak cocok.');
      return;
    }
    setBusy(true);
    setError('');
    const r = await completePasswordChange(pw);
    setBusy(false);
    if (!r.ok) setError(r.error ?? 'Gagal mengubah kata sandi.');
  };

  const canSubmit = pw.length >= 10 && pw === pw2 && !busy;

  return (
    <Card>
      <CardHeader title="Atur kata sandi baru" subtitle={session?.displayName ?? 'Akun Anda'} />
      <CardBody>
        <Stack gap={12}>
          <p style={muted}>
            Kata sandi yang diberikan tim IT adalah kata sandi sementara. Atur kata sandi pribadi
            sekarang — minimal 10 karakter dan harus berisi huruf serta angka.
          </p>
          <div>
            <label style={labelStyle}>Kata sandi baru</label>
            <input
              style={field}
              type="password"
              value={pw}
              onChange={(e) => setPw(e.target.value)}
              autoComplete="new-password"
            />
          </div>
          <div>
            <label style={labelStyle}>Ulangi kata sandi</label>
            <input
              style={field}
              type="password"
              value={pw2}
              onChange={(e) => setPw2(e.target.value)}
              autoComplete="new-password"
            />
          </div>
          {error ? <StatusPill kind="danger">{error}</StatusPill> : null}
          <Button variant="primary" size="md" disabled={!canSubmit} onClick={() => void submit()}>
            {busy ? 'Memproses…' : 'Simpan & lanjutkan'}
          </Button>
        </Stack>
      </CardBody>
    </Card>
  );
}
