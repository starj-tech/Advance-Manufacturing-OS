import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Stack } from '@aether/ui-kit';
import { ChangePasswordForm, LoginForm, useSession } from '@aether/auth';
import { OnboardingWizard } from './onboarding/OnboardingWizard';
import { ProductShell } from './product/ProductShell';

const page: CSSProperties = {
  minHeight: '100vh',
  padding: 24,
  display: 'flex',
  justifyContent: 'center',
};

function FirstLoginReset() {
  const { signOut } = useSession();
  return (
    <div style={{ width: '100%', maxWidth: 420 }}>
      <Stack gap={12}>
        <ChangePasswordForm />
        <div>
          <Button variant="ghost" size="sm" onClick={signOut}>
            Keluar
          </Button>
        </div>
      </Stack>
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
    if (session.mustChangePassword) {
      return (
        <div style={page}>
          <FirstLoginReset />
        </div>
      );
    }
    return <ProductShell />;
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
