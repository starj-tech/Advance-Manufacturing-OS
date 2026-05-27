# AETHER-OS

> Industrial Operating System for advanced manufacturing — _Invisible yet Omnipresent_.

AETHER-OS adalah Industrial OS **berbasis web sepenuhnya** yang dirancang menjadi sistem saraf pusat bagi pabrik manufaktur — dari workshop kecil hingga gigafactory. Tidak ada yang perlu diunduh atau di-install: buka di browser, login, dan shell yang sesuai dengan role Anda (Developer, Executive, Manager, Employee) langsung termuat. Backend berjalan di Supabase (Postgres + Deno Edge Functions).

## Tujuh Pilar Arsitektur

1. **Web-First SPA** — satu bundel React + Vite + TypeScript yang berjalan di browser modern mana pun. Tanpa unduhan, tanpa installer, tanpa native runtime. Lazy chunk per shell menjaga initial load tetap kecil (< 350KB gz, di-enforce CI).
2. **Cloud-Backed, Offline-Aware** — Supabase Postgres adalah source of truth. Klien meng-cache dan mengantre tulisan secara lokal sehingga UI tetap responsif di Wi-Fi pabrik yang tidak stabil, lalu rekonsiliasi saat koneksi pulih.
3. **Zero-Knowledge Encryption** — enkripsi sisi-klien lewat Web Crypto API. Master key tenant tidak pernah dikirim ke server; server hanya melihat ciphertext untuk data sensitif (formula, finansial, IP).
4. **Dynamic Shell** — satu codebase yang bertransformasi berdasarkan role setelah otentikasi. Lazy chunk per shell, shared chrome konsisten, permission di-enforce server lewat RLS Supabase.
5. **Module Injection** — fitur tambahan = data terdaftar + ditandatangani (ed25519), diverifikasi di Edge Function, lalu hot-load di Web Worker dengan capability gating. Update tanpa menghentikan produksi.
6. **Industrial Protocols via Edge/Gateway** — browser tidak bicara OPC-UA/MQTT secara langsung. Gateway on-prem atau Edge Function menjembatani data protokol ke Supabase; SPA membacanya lewat HTTPS + Supabase Realtime. Domain layer tidak peduli sumbernya Siemens S7 atau MQTT broker.
7. **Safety-First Employee Shell** — SOS dieskalasi lewat Supabase Realtime → Edge Function (SMS/Slack/webhook). Geofencing memakai browser Geolocation API. Glove-Friendly UI dengan target sentuh 56dp dan kontras AAA.

## Quickstart

### Prasyarat

- Node.js ≥ 20 (lihat `.nvmrc`)
- pnpm ≥ 9
- Supabase CLI (untuk pengembangan lokal)

### Setup

```bash
# Install dependencies
pnpm install

# Jalankan Supabase lokal
supabase start
supabase db reset

# Jalankan web app dalam mode dev
pnpm -F @aether/web dev
```

Buka URL yang ditampilkan Vite (default `http://localhost:5173`).

### Build produksi

```bash
pnpm -F @aether/web build      # menghasilkan SPA statis di apps/web/dist
pnpm -F @aether/web preview    # serve hasil build secara lokal
```

Output `apps/web/dist` adalah situs statis yang bisa di-deploy ke host static mana pun (Cloudflare Pages, Netlify, Vercel, S3+CDN, dll).

### Lint & Test

```bash
pnpm lint
pnpm typecheck
pnpm test
```

## Layout Repositori

```
.
├── apps/web/                 # Web SPA utama (React + Vite + TypeScript)
├── packages/                 # JS/TS shared packages
│   ├── ui-kit/               # Design system standar
│   ├── glove-kit/            # Touch-first variant utk Employee Shell
│   ├── shell-runtime/        # Dynamic Shell engine
│   ├── module-sdk/           # SDK utk vendor modul
│   ├── rpc-contracts/        # Shared Zod schemas + TS types (kontrak HTTP)
│   └── eslint-config/        # Shared lint config
├── supabase/                 # Postgres migrations + Deno Edge Functions
└── docs/architecture/        # Deep-dive arsitektur
```

## Status

Frontend SPA dan backend Supabase (Postgres + Edge Functions) adalah deliverable produk. Logika domain yang harus berjalan di server diimplementasikan sebagai Deno Edge Functions (lihat `supabase/functions/compliance-attest` sebagai template TypeScript yang sudah jadi); logika yang aman di klien hidup di dalam SPA.

Lihat `docs/architecture/README.md` untuk peta deep-dive.

## License

Proprietary © starj-tech
