import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { signInWithCompanyId } from './sign-in';

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

export interface LoginFormProps {
  /** App name shown in the header, e.g. "Aplikasi Utama" / "Aplikasi IT". */
  appName: string;
}

/**
 * Company-ID + Username + password login. On success, the SessionProvider's
 * auth listener updates the session and the app re-renders into the product.
 */
export function LoginForm({ appName }: LoginFormProps) {
  const [companyId, setCompanyId] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    setError('');
    try {
      const { error: signInError } = await signInWithCompanyId({ companyId, username, password });
      if (signInError) {
        setError('Login gagal. Periksa Company ID, Username, dan kata sandi.');
      }
    } catch {
      setError('Backend belum dikonfigurasi.');
    } finally {
      setBusy(false);
    }
  };

  const canSubmit = companyId.trim() && username.trim() && password && !busy;

  return (
    <Card>
      <CardHeader title="Masuk ke AETHER-OS" subtitle={appName} />
      <CardBody>
        <Stack gap={12}>
          <div>
            <label style={labelStyle}>Company ID</label>
            <input
              style={field}
              value={companyId}
              onChange={(e) => setCompanyId(e.target.value)}
              placeholder="ACME-7Q2F"
              autoComplete="organization"
            />
          </div>
          <div>
            <label style={labelStyle}>Username</label>
            <input
              style={field}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="ani"
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
          <Button variant="primary" size="md" disabled={!canSubmit} onClick={() => void submit()}>
            {busy ? 'Memproses…' : 'Masuk'}
          </Button>
        </Stack>
      </CardBody>
    </Card>
  );
}
