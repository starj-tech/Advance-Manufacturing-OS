/**
 * Design tokens. Standard density (manager / executive / developer shells).
 * Glove-kit overrides these for the employee shell — see @aether/glove-kit.
 */
export const tokens = {
  touchTargetMin: 36,
  fontSizeBody: 14,
  fontSizePrimary: 16,
  contrastMin: 4.5, // WCAG AA
  radiusSm: 4,
  radiusMd: 8,
  radiusLg: 12,
  spacingXs: 4,
  spacingSm: 8,
  spacingMd: 12,
  spacingLg: 16,
  spacingXl: 24,
  colors: {
    bg: 'var(--aether-bg)',
    bgElevated: 'var(--aether-bg-elevated)',
    fg: 'var(--aether-fg)',
    fgMuted: 'var(--aether-fg-muted)',
    border: 'var(--aether-border)',
    accent: 'var(--aether-accent)',
    success: 'var(--aether-success)',
    warning: 'var(--aether-warning)',
    danger: 'var(--aether-danger)',
  },
} as const;

export type Tokens = typeof tokens;
