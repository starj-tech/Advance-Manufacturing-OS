import { useState } from 'react';
import { GloveButton, gloveTokens } from '@aether/glove-kit';

export function ClockPage() {
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
        {clockedIn ? 'On shift' : 'Off shift'}
      </h1>
      {since !== null ? (
        <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: gloveTokens.fontSizeBody }}>
          Clocked in at {new Date(since).toLocaleTimeString()}
        </p>
      ) : null}
      <GloveButton variant={clockedIn ? 'danger' : 'primary'} fullWidth onClick={toggle}>
        {clockedIn ? 'Clock out' : 'Clock in'}
      </GloveButton>
      <p
        style={{
          margin: 0,
          fontSize: 13,
          color: 'var(--aether-fg-muted)',
          textAlign: 'center',
        }}
      >
        Geofence + presence broadcast wires up in PR #5.
      </p>
    </div>
  );
}
