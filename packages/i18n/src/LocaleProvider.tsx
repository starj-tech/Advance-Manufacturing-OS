import { createContext, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { LOCALES } from './locales';
import type { LocaleDescriptor } from './locales';

export type Locale = LocaleDescriptor;

export interface LocaleContextValue {
  locale: Locale;
  setLocale: (tag: string) => void;
  available: ReadonlyArray<Locale>;
}

const LocaleContext = createContext<LocaleContextValue | null>(null);

const FALLBACK = LOCALES[0]!;

interface ProviderProps {
  initialTag?: string;
  children: ReactNode;
}

export function LocaleProvider({ initialTag, children }: ProviderProps) {
  const [locale, setLocaleInternal] = useState<Locale>(() => {
    if (initialTag) {
      const match = LOCALES.find((l) => l.tag === initialTag);
      if (match) return match;
    }
    return FALLBACK;
  });

  const value = useMemo<LocaleContextValue>(
    () => ({
      locale,
      available: LOCALES,
      setLocale: (tag) => {
        const match = LOCALES.find((l) => l.tag === tag);
        if (match) setLocaleInternal(match);
      },
    }),
    [locale],
  );

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale(): LocaleContextValue {
  const ctx = useContext(LocaleContext);
  if (!ctx) {
    throw new Error('useLocale must be used within <LocaleProvider>');
  }
  return ctx;
}
