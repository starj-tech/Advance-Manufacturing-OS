import { useTranslation } from '@aether/i18n';
import { Placeholder } from '../_shared/Placeholder';

export function MaintenancePage() {
  const { t } = useTranslation();
  return (
    <Placeholder
      title={t('page.maintenance.title')}
      description={t('page.maintenance.desc')}
      shipsIn="PR #3 + analytics module (PR #7)"
    />
  );
}
