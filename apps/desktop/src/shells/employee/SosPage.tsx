import { useState } from 'react';
import { SosButton, gloveTokens } from '@aether/glove-kit';
import { useTranslation } from '@aether/i18n';

export function SosPage() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<'idle' | 'sent' | 'failed'>('idle');

  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: gloveTokens.spacingLg,
        padding: gloveTokens.spacingLg,
      }}
    >
      <h1 style={{ margin: 0, fontSize: 32, fontWeight: 800 }}>{t('employee.sos.title')}</h1>
      <p
        style={{
          margin: 0,
          fontSize: gloveTokens.fontSizeBody,
          color: 'var(--aether-fg-muted)',
          textAlign: 'center',
          maxWidth: 360,
        }}
      >
        {t('employee.sos.instructions')}
      </p>

      <SosButton
        onTrigger={() => {
          // PR #5 wires this to commands.safety.trigger_sos via @tauri-apps/api/core::invoke.
          setStatus('sent');
          setTimeout(() => setStatus('idle'), 4000);
        }}
      />

      {status === 'sent' ? (
        <div
          role="status"
          style={{
            padding: '12px 16px',
            background: 'rgba(16, 185, 129, 0.15)',
            color: 'var(--aether-success)',
            borderRadius: gloveTokens.radiusMd,
            fontSize: gloveTokens.fontSizeBody,
            fontWeight: 600,
          }}
        >
          {t('employee.sos.alertSent')}
        </div>
      ) : null}
    </div>
  );
}
