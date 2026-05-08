import { forwardRef } from 'react';
import type { ButtonHTMLAttributes } from 'react';
import { gloveTokens } from './tokens';

export interface GloveButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'danger';
  fullWidth?: boolean;
}

const VARIANT = {
  primary: { bg: 'var(--aether-accent)', fg: '#fff' },
  secondary: { bg: 'var(--aether-bg-elevated)', fg: 'var(--aether-fg)' },
  danger: { bg: 'var(--aether-danger)', fg: '#fff' },
} as const;

export const GloveButton = forwardRef<HTMLButtonElement, GloveButtonProps>(
  ({ variant = 'secondary', fullWidth, style, children, ...rest }, ref) => {
    const v = VARIANT[variant];
    return (
      <button
        ref={ref}
        {...rest}
        style={{
          background: v.bg,
          color: v.fg,
          minHeight: gloveTokens.touchTargetMin,
          minWidth: gloveTokens.touchTargetMin,
          padding: `${gloveTokens.spacingSm}px ${gloveTokens.spacingMd}px`,
          fontSize: gloveTokens.fontSizePrimary,
          fontWeight: 600,
          borderRadius: gloveTokens.radiusMd,
          border: '2px solid transparent',
          width: fullWidth ? '100%' : undefined,
          ...style,
        }}
      >
        {children}
      </button>
    );
  },
);
GloveButton.displayName = 'GloveButton';
