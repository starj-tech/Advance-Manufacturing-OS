import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { signInWithEmail } from '@aether/auth';

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

/** Vendor-staff email/password login (platform_admins realm). */
export function ConsoleLogin() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    setError('');
    try {
      const { error: e } = await signInWithEmail(email, password);
      if (e) setError('Login gagal.');
    } catch {
      setError('Backend belum dikonfigurasi.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card>
      <CardHeader title="Konsol Vendor" subtitle="Khusus staf AETHER-OS" />
      <CardBody>
        <Stack gap={12}>
          <div>
            <label style={labelStyle}>Email</label>
            <input
              style={field}
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="username"
            />
          </div>
          <div>
            <label style={labelStyle}>Kata sandi</label>
            <input
              style={field}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
            />
          </div>
          {error ? <StatusPill kind="danger">{error}</StatusPill> : null}
          <Button
            variant="primary"
            size="md"
            disabled={!email || !password || busy}
            onClick={() => void submit()}
          >
            {busy ? 'Memproses…' : 'Masuk'}
          </Button>
        </Stack>
      </CardBody>
    </Card>
  );
}
