import { describe, expect, it } from 'vitest';
import { BASE_LOCALE, RESOURCES, interpolate, resolveMessage } from './resources';
import { LOCALES } from './locales';

describe('resolveMessage', () => {
  it('returns the active locale value when the key exists there', () => {
    expect(resolveMessage('nl-NL', 'topbar.signOut')).toBe('Afmelden');
    expect(resolveMessage('de-DE', 'topbar.signOut')).toBe('Abmelden');
    expect(resolveMessage('ja-JP', 'employee.nav.clock')).toBe('打刻');
  });

  it('falls back to en-US when the active locale lacks the key', () => {
    // Inject a deliberately partial locale at runtime to prove the
    // fallback path without weakening the shipped bundles.
    const partialTag = 'xx-PARTIAL';
    RESOURCES[partialTag] = { 'topbar.signOut': 'OnlyThisKey' };
    try {
      expect(resolveMessage(partialTag, 'topbar.signOut')).toBe('OnlyThisKey');
      expect(resolveMessage(partialTag, 'employee.nav.tasks')).toBe(
        RESOURCES[BASE_LOCALE]!['employee.nav.tasks'],
      );
    } finally {
      delete RESOURCES[partialTag];
    }
  });

  it('falls back to the raw key when no bundle defines it', () => {
    expect(resolveMessage('nl-NL', 'totally.unknown.key')).toBe('totally.unknown.key');
    expect(resolveMessage('en-US', 'totally.unknown.key')).toBe('totally.unknown.key');
  });

  it('falls back to en-US for an unregistered locale tag', () => {
    expect(resolveMessage('fr-FR', 'topbar.signOut')).toBe(
      RESOURCES[BASE_LOCALE]!['topbar.signOut'],
    );
  });
});

describe('interpolate', () => {
  it('substitutes a single placeholder', () => {
    expect(interpolate('industry · {name}', { name: 'Food & Beverage' })).toBe(
      'industry · Food & Beverage',
    );
  });

  it('substitutes every occurrence and coerces numbers', () => {
    expect(interpolate('{count} of {count}', { count: 3 })).toBe('3 of 3');
  });

  it('returns the template untouched when no params are given', () => {
    expect(interpolate('no placeholders here')).toBe('no placeholders here');
  });

  it('leaves unmatched placeholders intact', () => {
    expect(interpolate('hello {missing}', { other: 'x' })).toBe('hello {missing}');
  });
});

describe('catalog integrity', () => {
  const baseKeys = Object.keys(RESOURCES[BASE_LOCALE]!).sort();

  it('registers a bundle for every LOCALES descriptor', () => {
    for (const { tag } of LOCALES) {
      expect(RESOURCES[tag], `missing resource bundle for ${tag}`).toBeDefined();
    }
  });

  it('every locale has full key parity with the en-US base', () => {
    // A missing key would silently fall back to English. Pin parity
    // so a half-translated locale fails CI instead of shipping mixed
    // language — this is exactly the bug the catalog was built to kill.
    for (const tag of Object.keys(RESOURCES)) {
      const keys = Object.keys(RESOURCES[tag]!).sort();
      expect(keys, `key drift in ${tag}`).toEqual(baseKeys);
    }
  });

  it('placeholder tokens match the base for every translated key', () => {
    const tokensOf = (s: string) => (s.match(/\{[^}]+\}/g) ?? []).sort();
    const base = RESOURCES[BASE_LOCALE]!;
    for (const tag of Object.keys(RESOURCES)) {
      const bundle = RESOURCES[tag]!;
      for (const key of baseKeys) {
        expect(tokensOf(bundle[key]!), `placeholder drift in ${tag} → ${key}`).toEqual(
          tokensOf(base[key]!),
        );
      }
    }
  });
});
