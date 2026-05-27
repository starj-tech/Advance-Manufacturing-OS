import { useState } from 'react';
import { GloveButton, gloveTokens } from '@aether/glove-kit';
import { useTranslation } from '@aether/i18n';

export function ClockPage() {
  const { t } = useTranslation();
  const [clockedIn, setClockedIn] = useState(false);
  const [since, setSince] = useState<number | null>(null);

  const toggle = () => {
    if (clockedIn) {
      setClockedIn(false);
      setSince(null);
    } else {
      setClockedIn(true);
      setSince(Date.now());
    }
  };

  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: gloveTokens.spacingLg,
      }}
    >
      <h1 style={{ margin: 0, fontSize: 28, fontWeight: 800 }}>
        {clockedIn ? t('employee.clock.onShift') : t('employee.clock.offShift')}
      </h1>
      {since !== null ? (
        <p
          style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: gloveTokens.fontSizeBody }}
        >
          {t('employee.clock.clockedInAt', { time: new Date(since).toLocaleTimeString() })}
        </p>
      ) : null}
      <GloveButton variant={clockedIn ? 'danger' : 'primary'} fullWidth onClick={toggle}>
        {clockedIn ? t('employee.clock.clockOut') : t('employee.clock.clockIn')}
      </GloveButton>
      <p
        style={{
          margin: 0,
          fontSize: 13,
          color: 'var(--aether-fg-muted)',
          textAlign: 'center',
        }}
      >
        {t('employee.clock.geofenceNote')}
      </p>
    </div>
  );
}
