// AETHER-OS — send-welcome-email edge function (Resend).
//
// Sends the post-payment welcome email: congratulations, the Company ID, the
// list of provisioned accounts (name + username + role), and login + invoice
// pointers. Initial passwords are NOT emailed in bulk — each user sets theirs
// on first login (tenant_users.must_change_password); the IT app distributes
// or resets credentials. Honors "account info delivered" without shipping
// plaintext secrets to a shared inbox.
//
// Secrets (Deno.env): RESEND_API_KEY, RESEND_FROM. Missing key => a safe
// no-op (200 { sent: false }) so it never blocks provisioning.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export interface WelcomeAccount {
  name: string;
  username: string;
  role: string;
}

export interface WelcomeInput {
  companyName: string;
  companyId: string;
  tier: string;
  billingEmail: string;
  accounts: WelcomeAccount[];
  loginUrl?: string;
}

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/** Build the welcome email subject + HTML + text. Pure (unit-tested). */
export function buildWelcomeEmail(input: WelcomeInput): {
  subject: string;
  html: string;
  text: string;
} {
  const login = input.loginUrl ?? 'https://app.aether-os.com';
  const subject = `Selamat datang di AETHER-OS — ${input.companyName}`;

  const rows = input.accounts
    .map(
      (a) =>
        `<tr><td>${esc(a.name)}</td><td><code>${esc(a.username)}</code></td><td>${esc(a.role)}</td></tr>`,
    )
    .join('');

  const html = `
<h1>Selamat datang, ${esc(input.companyName)}!</h1>
<p>Langganan <strong>${esc(input.tier)}</strong> Anda aktif. Faktur terlampir di akun Stripe Anda.</p>
<p><strong>Company ID:</strong> <code>${esc(input.companyId)}</code></p>
<p>${input.accounts.length} akun telah dibuat. Login di <a href="${login}">${login}</a> dengan
Company ID + Username + kata sandi (wajib diganti saat login pertama).</p>
<table border="1" cellpadding="6" cellspacing="0">
  <thead><tr><th>Nama</th><th>Username</th><th>Peran</th></tr></thead>
  <tbody>${rows}</tbody>
</table>
<p>Tim IT Anda dapat mengelola akun & reset kata sandi di Aplikasi IT.</p>`.trim();

  const text = [
    `Selamat datang, ${input.companyName}!`,
    `Langganan ${input.tier} aktif. Company ID: ${input.companyId}.`,
    `${input.accounts.length} akun dibuat. Login di ${login} (Company ID + Username + sandi, ganti saat pertama login).`,
    ...input.accounts.map((a) => `- ${a.name} | ${a.username} | ${a.role}`),
  ].join('\n');

  return { subject, html, text };
}

export async function handler(req: Request): Promise<Response> {
  const json = (body: unknown, status = 200) =>
    new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

  if (req.method !== 'POST') return json({ error: 'method not allowed' }, 405);

  let input: WelcomeInput;
  try {
    input = (await req.json()) as WelcomeInput;
  } catch {
    return json({ error: 'invalid body' }, 400);
  }
  if (!input.billingEmail || !input.companyId) return json({ error: 'missing fields' }, 400);

  const apiKey = Deno.env.get('RESEND_API_KEY');
  const from = Deno.env.get('RESEND_FROM') ?? 'AETHER-OS <welcome@aether-os.com>';
  const email = buildWelcomeEmail(input);

  // Safe no-op when Resend is not configured — provisioning must not block.
  if (!apiKey)
    return json({ sent: false, reason: 'resend not configured', subject: email.subject });

  const res = await fetch('https://api.resend.com/emails', {
    method: 'POST',
    headers: { authorization: `Bearer ${apiKey}`, 'content-type': 'application/json' },
    body: JSON.stringify({
      from,
      to: [input.billingEmail],
      subject: email.subject,
      html: email.html,
      text: email.text,
    }),
  });
  if (!res.ok) return json({ sent: false, status: res.status }, 502);
  return json({ sent: true });
}

if (import.meta.main) {
  serve(handler);
}
