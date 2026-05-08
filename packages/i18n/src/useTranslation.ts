import { useLocale } from './LocaleProvider';

/**
 * Minimal translation hook. PR #6 plugs in a real resource bundle
 * loader (lazy fetch from Supabase Storage or bundled per-locale JSON);
 * for the skeleton we return the key as-is so the UI doesn't break
 * when a translation is missing.
 */
export function useTranslation() {
  const { locale } = useLocale();
  const t = (key: string, params?: Record<string, string | number>) => {
    let s = key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        s = s.replaceAll(`{${k}}`, String(v));
      }
    }
    return s;
  };
  return { t, locale };
}
