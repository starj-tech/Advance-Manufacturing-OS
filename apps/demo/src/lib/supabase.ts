import { createClient, type SupabaseClient } from '@supabase/supabase-js';

const url = import.meta.env.VITE_SUPABASE_URL;
const anonKey = import.meta.env.VITE_SUPABASE_ANON_KEY;

/**
 * True when both Supabase connection vars are present at build time. When
 * false the app runs in demo mode: local data and the dev-mode role picker.
 */
export const isSupabaseConfigured = Boolean(url && anonKey);

/**
 * Shared Supabase client, or `null` when the app is built without
 * VITE_SUPABASE_URL / VITE_SUPABASE_ANON_KEY. Callers must treat a null
 * client as demo mode and fall back to local data rather than throwing.
 */
export const supabase: SupabaseClient | null = isSupabaseConfigured
  ? createClient(url as string, anonKey as string, {
      auth: { persistSession: true, autoRefreshToken: true },
    })
  : null;
