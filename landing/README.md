# Landing page

Marketing site for **Defender Control**, built with [Astro](https://astro.build) and the
Vercel / Geist design system. Ships as static HTML with near-zero client JS (only the
hero carousel hydrates).

## Develop

```sh
cd landing
npm install
npm run dev        # http://localhost:4321
```

## Build

```sh
npm run build      # outputs static site to dist/
npm run preview    # preview the production build
```

## Deploy on Vercel

1. Import the repo at [vercel.com/new](https://vercel.com/new).
2. **Root Directory** → `landing`
3. Vercel auto-detects Astro (Framework Preset → `Astro`); build command `astro build`, output `dist`.
4. Deploy.

## Structure

```
landing/
├─ public/assets/        logo + the four language screenshots
├─ src/
│  ├─ layouts/Base.astro  <head>, fonts, meta
│  ├─ components/         Nav, Hero (carousel), SpecBar, Features,
│  │                      Pipeline, Languages, Tech, Notice, CTA, Footer
│  ├─ pages/index.astro   composes the page
│  └─ styles/global.css   Geist / Vercel design system
├─ astro.config.mjs
└─ package.json
```
