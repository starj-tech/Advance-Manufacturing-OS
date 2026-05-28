import { useMemo, useState } from 'react';
import type { ChangeEvent, CSSProperties } from 'react';
import { Button, Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';
import { INDUSTRY_CATALOG, industrySlugs } from '@aether/industry-catalog';
import { TIERS, priceLabel, type BillingCycle, type TierSlug } from './tiers';
import { parseRosterCsv } from './roster';

const STEPS = ['Perusahaan', 'Industri', 'Karyawan', 'Paket', 'Tinjau'] as const;

const field: CSSProperties = {
  width: '100%',
  padding: '10px 12px',
  fontSize: 14,
  background: 'var(--aether-bg)',
  color: 'var(--aether-fg)',
  border: '1px solid var(--aether-border)',
  borderRadius: 8,
  outline: 'none',
};
const labelStyle: CSSProperties = {
  display: 'block',
  fontSize: 12,
  color: 'var(--aether-fg-muted)',
  marginBottom: 6,
};
const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };

export function OnboardingWizard() {
  const [step, setStep] = useState(0);
  const [companyName, setCompanyName] = useState('');
  const [billingEmail, setBillingEmail] = useState('');
  const [region, setRegion] = useState('us-east');
  const [industry, setIndustry] = useState('');
  const [rosterText, setRosterText] = useState('');
  const [tier, setTier] = useState<TierSlug>('advanced-automata');
  const [cycle, setCycle] = useState<BillingCycle>('monthly');
  const [submitted, setSubmitted] = useState(false);

  const roster = useMemo(() => parseRosterCsv(rosterText), [rosterText]);

  const onFile = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setRosterText(await file.text());
  };

  const stepValid = (() => {
    if (step === 0) return companyName.trim().length > 0 && /.+@.+\..+/.test(billingEmail);
    if (step === 1) return industry.length > 0;
    if (step === 2) return roster.length > 0;
    return true;
  })();

  if (submitted) {
    const t = TIERS.find((x) => x.slug === tier)!;
    return (
      <Card>
        <CardHeader title="Pendaftaran disiapkan" subtitle={companyName} />
        <CardBody>
          <Stack gap={12}>
            <StatusPill kind="success">Siap ke pembayaran</StatusPill>
            <p style={{ margin: 0, fontSize: 14, lineHeight: 1.6 }}>
              {roster.length} akun akan dibuat untuk industri{' '}
              <strong>{INDUSTRY_CATALOG[industry]?.label ?? industry}</strong> pada paket{' '}
              <strong>{t.name}</strong> ({priceLabel(t, cycle)}). Langkah pembayaran Stripe +
              penyediaan akun otomatis + email selamat datang (Resend) terhubung oleh Edge Function
              <code> create-checkout</code> / <code>stripe-webhook</code> (Phase C backend).
            </p>
            <div>
              <Button variant="ghost" size="md" onClick={() => setSubmitted(false)}>
                Kembali ubah
              </Button>
            </div>
          </Stack>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader
        title="Daftarkan perusahaan Anda"
        subtitle={`Langkah ${step + 1} dari ${STEPS.length} · ${STEPS[step]}`}
      />
      <CardBody>
        <Stack gap={16}>
          {step === 0 ? (
            <Stack gap={12}>
              <div>
                <label style={labelStyle}>Nama perusahaan</label>
                <input
                  style={field}
                  value={companyName}
                  onChange={(e) => setCompanyName(e.target.value)}
                  placeholder="PT Contoh Manufaktur"
                />
              </div>
              <div>
                <label style={labelStyle}>Email penagihan</label>
                <input
                  style={field}
                  type="email"
                  value={billingEmail}
                  onChange={(e) => setBillingEmail(e.target.value)}
                  placeholder="billing@contoh.co.id"
                />
              </div>
              <div>
                <label style={labelStyle}>Region data</label>
                <select style={field} value={region} onChange={(e) => setRegion(e.target.value)}>
                  <option value="us-east">US East</option>
                  <option value="eu-west">EU West</option>
                  <option value="asia-se">Asia Tenggara</option>
                </select>
              </div>
            </Stack>
          ) : null}

          {step === 1 ? (
            <div>
              <label style={labelStyle}>Pilih industri ({industrySlugs().length} pilihan)</label>
              <select style={field} value={industry} onChange={(e) => setIndustry(e.target.value)}>
                <option value="">— pilih industri —</option>
                {industrySlugs().map((slug) => (
                  <option key={slug} value={slug}>
                    {INDUSTRY_CATALOG[slug]?.label ?? slug}
                  </option>
                ))}
              </select>
              {industry ? (
                <p style={{ ...muted, marginTop: 8 }}>
                  Kapabilitas: {INDUSTRY_CATALOG[industry]?.capabilities.join(', ')}
                </p>
              ) : null}
            </div>
          ) : null}

          {step === 2 ? (
            <Stack gap={12}>
              <p style={muted}>
                Unggah daftar karyawan (CSV: name,email,role,sub_role) atau tempel di bawah. Akun +
                kata sandi dibuat otomatis saat pembayaran selesai.
              </p>
              <input type="file" accept=".csv,text/csv" onChange={(e) => void onFile(e)} />
              <textarea
                style={{ ...field, minHeight: 120, fontFamily: 'monospace' }}
                value={rosterText}
                onChange={(e) => setRosterText(e.target.value)}
                placeholder={'name,email,role,sub_role\nAni,ani@contoh.co,manager,Production'}
              />
              {roster.length > 0 ? (
                <StatusPill kind="success">{roster.length} akun terbaca</StatusPill>
              ) : (
                <StatusPill kind="neutral">Belum ada baris valid</StatusPill>
              )}
            </Stack>
          ) : null}

          {step === 3 ? (
            <Stack gap={12}>
              <Stack direction="row" gap={8} align="center">
                <Button
                  variant={cycle === 'monthly' ? 'primary' : 'ghost'}
                  size="sm"
                  onClick={() => setCycle('monthly')}
                >
                  Bulanan
                </Button>
                <Button
                  variant={cycle === 'annual' ? 'primary' : 'ghost'}
                  size="sm"
                  onClick={() => setCycle('annual')}
                >
                  Tahunan (hemat ~2 bulan)
                </Button>
              </Stack>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                  gap: 12,
                }}
              >
                {TIERS.map((t) => {
                  const selected = t.slug === tier;
                  return (
                    <button
                      key={t.slug}
                      onClick={() => setTier(t.slug)}
                      style={{
                        textAlign: 'left',
                        padding: 16,
                        borderRadius: 12,
                        background: 'var(--aether-bg)',
                        border: `1px solid ${selected ? 'var(--aether-accent)' : 'var(--aether-border)'}`,
                        cursor: 'pointer',
                      }}
                    >
                      <Stack gap={6}>
                        <Stack direction="row" justify="space-between" align="center">
                          <strong style={{ fontSize: 15 }}>{t.name}</strong>
                          {t.highlighted ? <StatusPill kind="info">Populer</StatusPill> : null}
                        </Stack>
                        <span style={{ fontSize: 13, color: 'var(--aether-fg-muted)' }}>
                          {t.audience}
                        </span>
                        <span style={{ fontSize: 18, fontWeight: 700 }}>
                          {priceLabel(t, cycle)}
                        </span>
                        <span style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                          {t.seats}
                        </span>
                        <ul
                          style={{
                            margin: '6px 0 0',
                            paddingLeft: 16,
                            fontSize: 12,
                            lineHeight: 1.6,
                          }}
                        >
                          {t.features.map((f) => (
                            <li key={f}>{f}</li>
                          ))}
                        </ul>
                      </Stack>
                    </button>
                  );
                })}
              </div>
            </Stack>
          ) : null}

          {step === 4 ? (
            <Stack gap={8}>
              <p style={muted}>Tinjau sebelum lanjut ke pembayaran:</p>
              <ul style={{ margin: 0, paddingLeft: 18, fontSize: 14, lineHeight: 1.7 }}>
                <li>
                  Perusahaan: <strong>{companyName}</strong> ({billingEmail}, {region})
                </li>
                <li>
                  Industri: <strong>{INDUSTRY_CATALOG[industry]?.label ?? industry}</strong>
                </li>
                <li>
                  Akun: <strong>{roster.length}</strong>
                </li>
                <li>
                  Paket: <strong>{TIERS.find((t) => t.slug === tier)?.name}</strong> ·{' '}
                  {cycle === 'monthly' ? 'bulanan' : 'tahunan'}
                </li>
              </ul>
            </Stack>
          ) : null}

          <Stack direction="row" gap={8} justify="space-between">
            <Button
              variant="ghost"
              size="md"
              disabled={step === 0}
              onClick={() => setStep((s) => s - 1)}
            >
              Kembali
            </Button>
            {step < STEPS.length - 1 ? (
              <Button
                variant="primary"
                size="md"
                disabled={!stepValid}
                onClick={() => setStep((s) => s + 1)}
              >
                Lanjut
              </Button>
            ) : (
              <Button variant="primary" size="md" onClick={() => setSubmitted(true)}>
                Lanjut ke Pembayaran (Stripe)
              </Button>
            )}
          </Stack>
        </Stack>
      </CardBody>
    </Card>
  );
}
