import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { LoginForm, changePassword, useSession } from '@aether/auth';
import { OnboardingWizard } from './onboarding/OnboardingWizard';

const page: CSSProperties = {
  minHeight: '100vh',
  padding: 24,
  display: 'flex',
  justifyContent: 'center',
};
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

function ChangePasswordCard() {
  const [pw, setPw] = useState('');
  const [done, setDone] = useState(false);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      const { error } = await changePassword(pw);
      if (!error) setDone(true);
    } finally {
      setBusy(false);
    }
  };

  if (done) return <StatusPill kind="success">Kata sandi diperbarui</StatusPill>;
  return (
    <Stack gap={8}>
      <input
        style={field}
        type="password"
        value={pw}
        onChange={(e) => setPw(e.target.value)}
        placeholder="Kata sandi baru"
        autoComplete="new-password"
      />
      <div>
        <Button
          variant="primary"
          size="sm"
          disabled={pw.length < 8 || busy}
          onClick={() => void submit()}
        >
          {busy ? 'Menyimpan…' : 'Simpan kata sandi'}
        </Button>
      </div>
    </Stack>
  );
}

function AuthedHome() {
  const { session, signOut } = useSession();
  if (!session) return null;
  const sub = [session.companyId, session.primaryRole, session.subRole].filter(Boolean).join(' · ');
  return (
    <div style={{ width: '100%', maxWidth: 560 }}>
      <Card>
        <CardHeader title={`Halo, ${session.displayName}`} subtitle={sub} />
        <CardBody>
          <Stack gap={12}>
            <p style={{ margin: 0, fontSize: 14, lineHeight: 1.6 }}>
              Produk multi-peran (shell <strong>{session.primaryRole}</strong>) akan dimuat di sini
              dan disesuaikan per industri — Phase E.
            </p>
            {session.mustChangePassword ? (
              <Stack gap={8}>
                <StatusPill kind="warning">Wajib ganti kata sandi (login pertama)</StatusPill>
                <ChangePasswordCard />
              </Stack>
            ) : null}
            <div>
              <Button variant="ghost" size="md" onClick={signOut}>
                Keluar
              </Button>
            </div>
          </Stack>
        </CardBody>
      </Card>
    </div>
  );
}

export function MainView() {
  const { session, loading } = useSession();
  const [tab, setTab] = useState<'daftar' | 'masuk'>('daftar');

  if (loading) {
    return (
      <div style={page}>
        <p style={{ color: 'var(--aether-fg-muted)' }}>Memuat…</p>
      </div>
    );
  }

  if (session) {
    return (
      <div style={page}>
        <AuthedHome />
      </div>
    );
  }

  return (
    <div style={page}>
      <div style={{ width: '100%', maxWidth: 760 }}>
        <header
          style={{
            marginBottom: 16,
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}
        >
          <div>
            <h1 style={{ margin: 0, fontSize: 24 }}>AETHER-OS</h1>
            <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
              Aplikasi Utama
            </p>
          </div>
          <Stack direction="row" gap={8}>
            <Button
              variant={tab === 'daftar' ? 'primary' : 'ghost'}
              size="sm"
              onClick={() => setTab('daftar')}
            >
              Daftar
            </Button>
            <Button
              variant={tab === 'masuk' ? 'primary' : 'ghost'}
              size="sm"
              onClick={() => setTab('masuk')}
            >
              Masuk
            </Button>
          </Stack>
        </header>
        {tab === 'daftar' ? <OnboardingWizard /> : <LoginForm appName="Aplikasi Utama" />}
      </div>
    </div>
  );
}
