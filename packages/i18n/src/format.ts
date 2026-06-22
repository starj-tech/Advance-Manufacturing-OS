export interface MoneyAmount {
  /** Whole amount (not minor units). UI typically holds majors; the
   *  Rust side handles minor-unit precision for ledgers. */
  amount: number;
  currency: string;
}

export function formatMoney(localeTag: string, m: MoneyAmount): string {
  return new Intl.NumberFormat(localeTag, {
    style: 'currency',
    currency: m.currency,
    currencyDisplay: 'symbol',
  }).format(m.amount);
}

export function formatNumber(localeTag: string, value: number, fractionDigits = 2): string {
  return new Intl.NumberFormat(localeTag, {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(value);
}

export function formatDate(localeTag: string, value: Date | string): string {
  const d = value instanceof Date ? value : new Date(value);
  return new Intl.DateTimeFormat(localeTag, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(d);
}

export function formatRelative(
  localeTag: string,
  fromMs: number,
  toMs: number = Date.now(),
): string {
  const diff = (fromMs - toMs) / 1000;
  const abs = Math.abs(diff);
  const fmt = new Intl.RelativeTimeFormat(localeTag, { numeric: 'auto' });
  if (abs < 60) return fmt.format(Math.round(diff), 'second');
  if (abs < 3600) return fmt.format(Math.round(diff / 60), 'minute');
  if (abs < 86_400) return fmt.format(Math.round(diff / 3600), 'hour');
  if (abs < 604_800) return fmt.format(Math.round(diff / 86_400), 'day');
  return fmt.format(Math.round(diff / 604_800), 'week');
}
