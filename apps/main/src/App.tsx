import { OnboardingWizard } from './onboarding/OnboardingWizard';

export function App() {
  return (
    <div style={{ minHeight: '100vh', padding: 24, display: 'flex', justifyContent: 'center' }}>
      <div style={{ width: '100%', maxWidth: 760 }}>
        <header style={{ marginBottom: 16 }}>
          <h1 style={{ margin: 0, fontSize: 24 }}>AETHER-OS</h1>
          <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
            Aplikasi Utama — daftarkan perusahaan, pilih industri & paket, lalu akses platform.
          </p>
        </header>
        <OnboardingWizard />
      </div>
    </div>
  );
}
