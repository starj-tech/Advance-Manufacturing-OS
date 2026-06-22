import type { CSSProperties, HTMLAttributes, ReactNode } from 'react';

export interface StackProps extends HTMLAttributes<HTMLDivElement> {
  direction?: 'row' | 'column';
  gap?: number;
  align?: CSSProperties['alignItems'];
  justify?: CSSProperties['justifyContent'];
  wrap?: boolean;
  children: ReactNode;
}

export function Stack({
  direction = 'column',
  gap = 8,
  align,
  justify,
  wrap,
  style,
  children,
  ...rest
}: StackProps) {
  return (
    <div
      {...rest}
      style={{
        display: 'flex',
        flexDirection: direction,
        gap,
        alignItems: align,
        justifyContent: justify,
        flexWrap: wrap ? 'wrap' : 'nowrap',
        ...style,
      }}
    >
      {children}
    </div>
  );
}
