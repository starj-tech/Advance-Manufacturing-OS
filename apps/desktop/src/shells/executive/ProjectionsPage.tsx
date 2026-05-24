import { useTranslation } from '@aether/i18n';
import { Placeholder } from '../_shared/Placeholder';

export function ProjectionsPage() {
  const { t } = useTranslation();
  return (
    <Placeholder
      title={t('page.projections.title')}
      description={t('page.projections.desc')}
      shipsIn="PR #7 — analytics module"
    />
  );
}
