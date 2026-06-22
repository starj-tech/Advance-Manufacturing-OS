/**
 * Glove-friendly tokens. Override of @aether/ui-kit tokens with larger
 * touch targets, higher contrast (WCAG AAA target), and bigger type.
 *
 * Rationale (ref docs/architecture/safety.md):
 *   - Apple HIG min touch target = 44dp; gloves reduce precision so
 *     we bump to 56dp.
 *   - Body type 16px+, primary actions 20px+ for outdoor visibility.
 *   - Contrast ratio ≥ 7:1 (WCAG AAA).
 */
export const gloveTokens = {
  touchTargetMin: 56,
  fontSizeBody: 18,
  fontSizePrimary: 22,
  contrastMin: 7.0,
  radiusSm: 8,
  radiusMd: 12,
  radiusLg: 16,
  spacingXs: 8,
  spacingSm: 12,
  spacingMd: 16,
  spacingLg: 24,
  spacingXl: 32,
} as const;

export type GloveTokens = typeof gloveTokens;
