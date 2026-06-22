import { TaskCard, gloveTokens } from '@aether/glove-kit';
import { useTranslation } from '@aether/i18n';

const TASKS = [
  {
    id: 't1',
    title: 'PRESS-01 · 50 units stator core',
    subtitle: 'Cycle time target 38s — current 42s',
    status: 'running' as const,
  },
  {
    id: 't2',
    title: 'CNC-12 · QC sample WO-0042',
    subtitle: 'Pull 1 unit per 25 produced',
    status: 'pending' as const,
  },
  {
    id: 't3',
    title: 'WELD-04 · Reset thermal trip',
    subtitle: 'Authorize via supervisor key',
    status: 'blocked' as const,
  },
];

export function TasksPage() {
  const { t } = useTranslation();
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: gloveTokens.spacingMd }}>
      <h1
        style={{
          margin: 0,
          fontSize: 28,
          fontWeight: 800,
          letterSpacing: '-0.01em',
        }}
      >
        {t('employee.tasks.title')}
      </h1>
      <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: gloveTokens.fontSizeBody }}>
        {t('employee.tasks.subtitle')}
      </p>
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: gloveTokens.spacingSm,
          marginTop: 8,
        }}
      >
        {TASKS.map((t) => (
          <TaskCard key={t.id} title={t.title} subtitle={t.subtitle} status={t.status} />
        ))}
      </div>
    </div>
  );
}
