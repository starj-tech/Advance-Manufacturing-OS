import type { CSSProperties, HTMLAttributes, ReactNode } from 'react';
import { tokens } from './tokens';

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  padded?: boolean;
}

export function Card({ padded = true, style, children, ...rest }: CardProps) {
  return (
    <div
      {...rest}
      style={{
        background: tokens.colors.bgElevated,
        border: `1px solid ${tokens.colors.border}`,
        borderRadius: tokens.radiusLg,
        padding: padded ? tokens.spacingLg : 0,
        display: 'flex',
        flexDirection: 'column',
        ...style,
      }}
    >
      {children}
    </div>
  );
}

const sectionBase: CSSProperties = {
  paddingLeft: tokens.spacingLg,
  paddingRight: tokens.spacingLg,
};

export function CardHeader({
  children,
  title,
  subtitle,
}: {
  children?: ReactNode;
  title?: ReactNode;
  subtitle?: ReactNode;
}) {
  return (
    <header
      style={{
        ...sectionBase,
        paddingTop: tokens.spacingLg,
        paddingBottom: tokens.spacingMd,
        borderBottom: `1px solid ${tokens.colors.border}`,
      }}
    >
      {title ? <h3 style={{ margin: 0, fontSize: 16, fontWeight: 600 }}>{title}</h3> : null}
      {subtitle ? (
        <p style={{ margin: '4px 0 0', color: tokens.colors.fgMuted, fontSize: 13 }}>{subtitle}</p>
      ) : null}
      {children}
    </header>
  );
}

export function CardBody({ children, style }: { children: ReactNode; style?: CSSProperties }) {
  return (
    <div
      style={{
        ...sectionBase,
        paddingTop: tokens.spacingMd,
        paddingBottom: tokens.spacingMd,
        ...style,
      }}
    >
      {children}
    </div>
  );
}

export function CardFooter({ children }: { children: ReactNode }) {
  return (
    <footer
      style={{
        ...sectionBase,
        paddingTop: tokens.spacingMd,
        paddingBottom: tokens.spacingLg,
        borderTop: `1px solid ${tokens.colors.border}`,
        display: 'flex',
        gap: tokens.spacingSm,
      }}
    >
      {children}
    </footer>
  );
}
