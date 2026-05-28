import { describe, it, expect } from 'vitest';
import { isSupabaseConfigured, supabase } from './supabase';

describe('supabase client', () => {
  it('falls back to demo mode when env vars are absent', () => {
    // The test runner sets no VITE_SUPABASE_* vars, so the client must be
    // null and callers must treat that as demo mode.
    expect(isSupabaseConfigured).toBe(false);
    expect(supabase).toBeNull();
  });
});
