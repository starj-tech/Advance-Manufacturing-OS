import { useTranslation } from '@aether/i18n';
import { Placeholder } from '../_shared/Placeholder';

export function RosterPage() {
  const { t } = useTranslation();
  return (
    <Placeholder
      title={t('page.roster.title')}
      description={t('page.roster.desc')}
      shipsIn="PR #5 — safety subsystem (presence) + HR module (later)"
    />
  );
}
