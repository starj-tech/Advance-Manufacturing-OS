import { useTranslation } from '@aether/i18n';
import { Placeholder } from '../_shared/Placeholder';

export function AuditLogPage() {
  const { t } = useTranslation();
  return (
    <Placeholder
      title={t('page.audit.title')}
      description={t('page.audit.desc')}
      shipsIn="PR #2 — sync engine"
    />
  );
}
