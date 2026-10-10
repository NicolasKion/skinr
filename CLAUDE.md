# skinr.space

Browse and view EVE Online ships in 3D. A Rust API (Axum + Postgres via sqlx)
serving JSON plus a `/cdn` passthrough to CCP's client CDNs, and a SvelteKit
frontend in `frontend/` (SSR, adapter-node, Tailwind v4, shadcn-svelte). The
setup mirrors MutaMarket (`../MutaMarket`): when in doubt about structure,
theme or tooling, do what it does.

## Rules

- One origin: the Vite dev proxy (`frontend/proxy-paths.ts`) and
  `deploy/Caddyfile.dev` route `/api` and `/cdn` to Axum. Keep them in sync and
  never point the browser at Axum's port.
- Ship data comes from the native SDE import (`cargo run --bin sde_import`),
  never hand-written fixtures in production.
- SDE text (descriptions and anything else from the game) is EVE HTML, not HTML.
  Store it raw; render it only through `src/eve_html.rs` (plain text, or styled
  segments the frontend draws with `$lib/components/eve-text.svelte`), never
  `{@html}`. That module follows CCP's guide
  (https://developers.eveonline.com/docs/guides/eve-html/): four entities only,
  significant whitespace, crossing and unclosed tags, alpha-first colors.
- No magic numbers: EVE ids, thresholds and tuning values become named consts
  with a comment saying what they are.
- Tests land with their code. Rust behavior tests drive the real router with
  external services replaced by local mocks on ephemeral ports.
- The design is Apple-style, not MutaMarket's (`frontend/src/routes/layout.css`):
  dark only, Inter, generous radii, a per-race `--tint`, `glass`/`display`/`reveal`
  utilities, borderless surfaces (no boxed cards). Motion uses `--ease-apple`.
- One shared 3D stage (`$lib/stage.svelte.ts`, `$lib/components/stage.svelte`)
  lives in the root layout at z-10 with a transparent canvas. Pages set its mode
  (hero, dock, hidden) and ship; UI that must sit above it uses z-20, anything
  behind it (the ship page's background type) sits at z-0. Never resize the canvas
  for a mode change; the dock is the same canvas scaled and clipped.
- From the blueprint chapter down the stage docks into the sheet's side
  elevation (`stage.blueprintFit` frames it at the drawing's scale) and stays
  there. The livery section shows a second view of the same ship instead:
  the renderer draws it into `renderer.liveryCanvas` each frame that section
  is on screen (`setLiveryView`, `$lib/components/livery-view.svelte`).
- The chapter effect on the ship page alone sets `stage.mode`.
- The site bar (`$lib/components/site-bar.svelte`) lives in the root layout;
  pages put their breadcrumb, section links and reset action into
  `$lib/nav.svelte.ts` while mounted and clear it on the way out.
- Write `backdrop-filter` without a hand-written `-webkit-` twin: the build's
  CSS minifier then keeps only the prefixed one, which Chrome ignores. It adds
  the prefix itself.
- The blueprint section's paper (`--blueprint-paper`) is a z-0 layer measured
  to the section, not the section's own background: page content sits at z-20,
  above the stage, and would hide the docked ship.
- Attribute icons on the sheet are the client's own (`/api/icons/{attribute}.png`,
  fetched by the SDE import); use `$lib/components/attribute-icon.svelte`.
- Liveries show the game's skin material icons (`/api/skins/{material}.png`).
  A livery without one is composed from the client's tintable icon layers
  (`/api/skins/layers/{layer}.png`) in CSS, not on a 2D canvas;
  `$lib/components/skin-icon.svelte` does both.
- A resource read that fails is reported by the renderer (`onMissing`) and
  collected in `stage.missing`; a broken ship glitches (the same post pass,
  `setGlitch`) and `$lib/components/stage-alert.svelte` says why. Runtime
  formats are opt-in: register any new one in `resMan.Register({ formats })`.
- The fitting window look is our own WebGPU pass (`frontend/src/lib/viewer/hologram.ts`)
  over the finished frame, not a runtime wireframe mode (WebGPU has none).
- The spec sheet reads only the dogma attributes listed in `src/sheet.rs`
  (`SHEET_ATTRIBUTES`); a test fails if it reads one that is not imported.
- Only one element per page may carry `view-transition-name: ship-name`
  (`stage.morphId` picks it).
- `@carbonenginejs/runtime` is pre-alpha. Workarounds live in
  `frontend/src/lib/viewer/renderer.ts` and `frontend/vite.config.ts`, each
  commented; remove them as the runtime fills the gaps.
- WebGPU needs a headed Chromium on Linux; `frontend/scripts/shot.js` opens one.

## Commands

- `docker compose up --build` serves everything on http://localhost:5200.
- `docker compose watch` keeps it current: it rebuilds and replaces the api
  or frontend container when their code changes (`develop.watch` in the
  compose file). The API and the SDE import share one image, so a rebuild
  after a new migration updates both.
- `docker compose up -d postgres`, `cargo run`, then `npm run dev -- --port 5200`
  in `frontend/` for development (Postgres on 127.0.0.1:5434, API on 3100).
- Before committing: `cargo fmt`, `cargo clippy --all-targets` (zero warnings),
  `cargo test`, and in `frontend/` `npm run check`, `npm run lint`,
  `npm run format:check`, `npm test`.
