export interface LocaleDescriptor {
  /** BCP-47 tag (en-US, id-ID, de-DE, ja-JP, ...). */
  tag: string;
  /** Native-language label shown in the locale switcher. */
  nativeName: string;
  /** Default ISO-4217 currency for this locale. */
  defaultCurrency: string;
  /** Default tax label shown on invoices in this locale. */
  defaultTaxLabel: string;
}

export const LOCALES: ReadonlyArray<LocaleDescriptor> = [
  {
    tag: 'en-US',
    nativeName: 'English (US)',
    defaultCurrency: 'USD',
    defaultTaxLabel: 'Sales tax',
  },
  { tag: 'en-GB', nativeName: 'English (UK)', defaultCurrency: 'GBP', defaultTaxLabel: 'VAT 20%' },
  {
    tag: 'id-ID',
    nativeName: 'Bahasa Indonesia',
    defaultCurrency: 'IDR',
    defaultTaxLabel: 'PPN 11%',
  },
  { tag: 'de-DE', nativeName: 'Deutsch', defaultCurrency: 'EUR', defaultTaxLabel: 'MwSt. 19%' },
  // Dutch (Nederlands). EU member → EUR; BTW standard rate 21%.
  { tag: 'nl-NL', nativeName: 'Nederlands', defaultCurrency: 'EUR', defaultTaxLabel: 'BTW 21%' },
  { tag: 'ja-JP', nativeName: '日本語', defaultCurrency: 'JPY', defaultTaxLabel: '消費税 10%' },
  { tag: 'zh-CN', nativeName: '中文', defaultCurrency: 'CNY', defaultTaxLabel: '增值税 13%' },
];
