import { useState } from 'react';
import { Card, CardBody, CardHeader, Stack, StatusPill, Button } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

interface ManagerHealingCard {
  at: string;
  title: string;
  plainSummary: string;
  urgency: 'ok' | 'watch' | 'needs-you';
  requiresConfirmation: boolean;
}

// Demo data; PR #5 fetches the healing-summary Edge Function over HTTP.
const CARDS: ManagerHealingCard[] = [
  {
    at: '2026-05-08 04:12',
    title: 'Sync queue caught up by itself',
    plainSummary:
      'Your factory was offline for 18 minutes around dawn. AETHER-OS held 120 events locally and reconciled them automatically when the link came back. No data loss; no action needed.',
    urgency: 'ok',
    requiresConfirmation: false,
  },
  {
    at: '2026-05-07 22:48',
    title: 'PRESS-01 OPC-UA reconnect storm — paused',
    plainSummary:
      "PRESS-01's PLC dropped its OPC-UA session repeatedly. AETHER-OS paused new subscriptions for 2 minutes to let the controller stabilize, then resumed. If this repeats, ask maintenance to check the network cable to the cabinet.",
    urgency: 'watch',
    requiresConfirmation: false,
  },
  {
    at: '2026-05-07 14:03',
    title: 'Telemetry storage at 90% — confirm trim?',
    plainSummary:
      "Your local telemetry buffer was 90% full. AETHER-OS can keep the most recent 48 hours of raw readings and downsample older data to 1-minute averages. Approve once and we'll handle this automatically from now on.",
    urgency: 'needs-you',
    requiresConfirmation: true,
  },
];

const PILL: Record<ManagerHealingCard['urgency'], 'success' | 'warning' | 'danger'> = {
  ok: 'success',
  watch: 'warning',
  'needs-you': 'danger',
};

export function SupportPage() {
  const { t } = useTranslation();
  const [confirmed, setConfirmed] = useState<Set<string>>(new Set());

  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.support.title')}</h1>
        <p
          style={{
            margin: '4px 0 0',
            color: 'var(--aether-fg-muted)',
            fontSize: 13,
            maxWidth: 720,
          }}
        >
          {t('page.support.intro')}
        </p>
      </header>

      {CARDS.map((c) => {
        const isConfirmed = confirmed.has(c.at);
        return (
          <Card key={c.at}>
            <CardHeader title={c.title} subtitle={c.at}>
              <div style={{ position: 'absolute', top: 16, right: 16 }}>
                <StatusPill kind={PILL[c.urgency]}>
                  {t(`page.support.urgency.${c.urgency}`)}
                </StatusPill>
              </div>
            </CardHeader>
            <CardBody>
              <p style={{ margin: 0, fontSize: 14, lineHeight: 1.55 }}>{c.plainSummary}</p>
              {c.requiresConfirmation ? (
                <Stack direction="row" gap={8} style={{ marginTop: 12 }}>
                  {isConfirmed ? (
                    <StatusPill kind="success">{t('page.support.approved')}</StatusPill>
                  ) : (
                    <>
                      <Button
                        variant="primary"
                        size="md"
                        onClick={() => setConfirmed((s) => new Set(s).add(c.at))}
                      >
                        {t('page.support.approveBtn')}
                      </Button>
                      <Button variant="ghost" size="md" disabled>
                        {t('page.support.talkToHuman')}
                      </Button>
                    </>
                  )}
                </Stack>
              ) : null}
            </CardBody>
          </Card>
        );
      })}
    </Stack>
  );
}
