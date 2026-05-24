import { useLocale } from './LocaleProvider';
import { interpolate, resolveMessage } from './resources';

/**
 * Translation hook backed by the in-binary resource catalog
 * (`resources.ts`). Resolves `key` against the active locale's
 * bundle, falling back to `en-US`, then to the key itself.
 * `{name}`-style placeholders in the resolved string are
 * substituted from `params`.
 *
 * The previous skeleton returned the key verbatim regardless of
 * locale — which is why switching to Deutsch / Nederlands left
 * every string in English. This version does the real lookup, so
 * a non-English locale now translates every keyed string and only
 * falls through to English for keys a given locale hasn't
 * translated yet.
 */
export function useTranslation() {
  const { locale } = useLocale();
  const t = (key: string, params?: Record<string, string | number>) =>
    interpolate(resolveMessage(locale.tag, key), params);
  return { t, locale };
}
