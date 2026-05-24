import { Card, CardBody, CardHeader, Stack } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';

export function DigitalTwinPage() {
  const { t } = useTranslation();
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>{t('page.digitalTwin.title')}</h1>
      <Card>
        <CardHeader
          title={t('page.digitalTwin.viewerTitle')}
          subtitle={t('page.digitalTwin.viewerSubtitle')}
        />
        <CardBody>
          <div
            style={{
              height: 480,
              borderRadius: 8,
              border: '1px dashed var(--aether-border)',
              display: 'grid',
              placeItems: 'center',
              color: 'var(--aether-fg-muted)',
              fontSize: 13,
            }}
          >
            {t('page.digitalTwin.placeholder')}
          </div>
        </CardBody>
      </Card>
    </Stack>
  );
}
