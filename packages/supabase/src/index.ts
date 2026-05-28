import { createClient, type SupabaseClient } from '@supabase/supabase-js';

// Self-contained access to Vite's build-time env so consuming packages don't
// need vite/client's ambient ImportMetaEnv to typecheck. Each app injects its
// own VITE_SUPABASE_* at build time.
const env = (import.meta as unknown as { env?: Record<string, string | undefined> }).env ?? {};
const url = env.VITE_SUPABASE_URL;
const anonKey = env.VITE_SUPABASE_ANON_KEY;

/**
 * True when both Supabase connection vars are present at build time. When
 * false, apps run in demo mode: local data and the dev-mode role picker.
 */
export const isSupabaseConfigured = Boolean(url && anonKey);

/**
 * Shared Supabase client for every AETHER-OS app, or `null` when an app is
 * built without VITE_SUPABASE_URL / VITE_SUPABASE_ANON_KEY. Callers must
 * treat a null client as demo mode and fall back to local data rather than
 * throwing. Each app injects its own env at build time via Vite.
 */
export const supabase: SupabaseClient | null = isSupabaseConfigured
  ? createClient(url as string, anonKey as string, {
      auth: { persistSession: true, autoRefreshToken: true },
    })
  : null;
