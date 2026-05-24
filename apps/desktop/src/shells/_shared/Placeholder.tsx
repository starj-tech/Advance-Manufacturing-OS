import { Card, CardBody, CardHeader, Stack } from '@aether/ui-kit';
import { useTranslation } from '@aether/i18n';
import type { ReactNode } from 'react';

export interface PlaceholderProps {
  title: string;
  description?: string;
  children?: ReactNode;
  /** Hint about which PR will land the real implementation. */
  shipsIn?: string;
}

export function Placeholder({ title, description, children, shipsIn }: PlaceholderProps) {
  const { t } = useTranslation();
  return (
    <Stack gap={16}>
      <Card>
        <CardHeader title={title} subtitle={description} />
        <CardBody>
          {children ?? (
            <p style={{ color: 'var(--aether-fg-muted)', fontSize: 13, margin: 0 }}>
              {t('common.skeletonNote')}
              {shipsIn ? ` (${shipsIn})` : ''}
            </p>
          )}
        </CardBody>
      </Card>
    </Stack>
  );
}
