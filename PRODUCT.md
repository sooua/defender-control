# Product

## Register

brand

## Users

Windows power users, developers, and IT/security tinkerers running Windows 10/11. They
land on the page after finding the project (GitHub, a forum, a search) and want to answer
three things fast: what does this do, is it safe/reversible, and how do I get it. They are
technical enough to read "TrustedInstaller token" and "registry backup" as reassurance,
not noise. Many arrive in zh / ja / ko, so the page itself is localized.

## Product Purpose

Defender Control is a tiny native Win32 (Rust) tool that disables and re-enables Windows
Defender from a single state-aware button, with a registry backup, a system restore point,
and a full reverse path. The landing page exists to make a security-sensitive tool read as
**legitimate and careful** rather than sketchy: it must communicate the reversibility and
the engineering seriousness that separate this from a "crack tool." Success = a visitor
trusts it enough to download or read the source.

## Brand Personality

Engineered and precise. Confident, restrained, infrastructure-grade — the voice of the
tool itself: native, ~300 KB, no runtime. Three words: **native, reversible, exact.**
Tone is matter-of-fact and technical; it states what the binary literally does and lets the
specifics (TI token, RegNotifyChangeKeyValue, IFEO) carry credibility. No hype, no
reassurance theater. The visual identity is committed to the Vercel / Geist monochrome
system by explicit decision (this is the brand, not a reflex pick).

## Anti-references

- **Generic SaaS template**: hero-metric blocks, identical icon-card grids, an eyebrow
  above every section, gradient text, numbered `01/02/03` scaffolding on non-sequences.
- **Sketchy "crack tool" energy**: neon, warez aesthetics, scammy oversized download
  buttons. The page must read as a legitimate engineering project.

## Design Principles

1. **Reversibility is the headline feature.** Every fear a visitor has ("will this brick my
   security?") is answered by backup + restore point + full reverse. Lead with it, don't bury it.
2. **Let the internals prove legitimacy.** Real mechanism names (TrustedInstaller, WdFilter
   altitude, PPL, IFEO) are the trust signal. Show the engineering instead of claiming quality.
3. **The product's discipline is the page's discipline.** Native, tiny, no dependencies — the
   page should feel the same: fast, monochrome, no decorative weight that isn't earning its place.
4. **Localized, not translated.** zh / ja / ko are first-class; copy and layout must hold up in
   all four, not just English.

## Accessibility & Inclusion

WCAG 2.1 AA. Body text ≥4.5:1 (verify the muted-gray scale against both light and dark
surfaces), large text ≥3:1, visible keyboard focus on every interactive element, full
keyboard operation of the theme toggle and language menu, and a `prefers-reduced-motion`
path for the carousel and all transitions. Dark and light themes must both pass.
