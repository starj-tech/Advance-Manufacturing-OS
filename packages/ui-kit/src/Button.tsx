import { forwardRef } from 'react';
import type { ButtonHTMLAttributes, CSSProperties } from 'react';
import { tokens } from './tokens';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  fullWidth?: boolean;
}

const sizeMap: Record<ButtonSize, { padding: string; fontSize: number; minHeight: number }> = {
  sm: { padding: '6px 10px', fontSize: 13, minHeight: 28 },
  md: { padding: '8px 14px', fontSize: 14, minHeight: 36 },
  lg: { padding: '10px 18px', fontSize: 15, minHeight: 40 },
};

const variantStyle = (v: ButtonVariant): CSSProperties => {
  switch (v) {
    case 'primary':
      return { background: tokens.colors.accent, color: '#fff', borderColor: tokens.colors.accent };
    case 'secondary':
      return {
        background: tokens.colors.bgElevated,
        color: tokens.colors.fg,
        borderColor: tokens.colors.border,
      };
    case 'ghost':
      return {
        background: 'transparent',
        color: tokens.colors.fg,
        borderColor: 'transparent',
      };
    case 'danger':
      return { background: tokens.colors.danger, color: '#fff', borderColor: tokens.colors.danger };
  }
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = 'secondary', size = 'md', fullWidth, style, children, ...rest }, ref) => {
    const sz = sizeMap[size];
    return (
      <button
        ref={ref}
        {...rest}
        style={{
          ...variantStyle(variant),
          padding: sz.padding,
          fontSize: sz.fontSize,
          minHeight: sz.minHeight,
          borderRadius: tokens.radiusMd,
          borderWidth: 1,
          borderStyle: 'solid',
          fontWeight: 500,
          width: fullWidth ? '100%' : undefined,
          transition: 'background 120ms ease',
          ...style,
        }}
      >
        {children}
      </button>
    );
  },
);
Button.displayName = 'Button';
