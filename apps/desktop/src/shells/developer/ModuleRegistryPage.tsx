import { useTranslation } from '@aether/i18n';
import { Placeholder } from '../_shared/Placeholder';

export function ModuleRegistryPage() {
  const { t } = useTranslation();
  return (
    <Placeholder
      title={t('page.modules.title')}
      description={t('page.modules.desc')}
      shipsIn="PR #4 — module runtime"
    />
  );
}
