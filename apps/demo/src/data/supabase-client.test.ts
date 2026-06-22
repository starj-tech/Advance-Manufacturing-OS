import { describe, it, expect } from 'vitest';
import { isSupabaseConfigured, supabase } from '@aether/supabase';

describe('@aether/supabase shared client (demo mode)', () => {
  it('falls back to demo mode when env vars are absent', () => {
    // The test runner sets no VITE_SUPABASE_* vars, so the shared client must
    // be null and callers must treat that as demo mode.
    expect(isSupabaseConfigured).toBe(false);
    expect(supabase).toBeNull();
  });
});
