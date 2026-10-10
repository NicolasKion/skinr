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
  dark by default with a light theme, Inter, generous radii, a per-race `--tint`,
  `glass`/`display`/`reveal` utilities, borderless surfaces (no boxed cards).
  Motion uses `--ease-apple`.
- Themes: `$lib/theme` keeps the choice in a cookie that `hooks.server.ts`
  turns into the class on `<html>` (`dark` or `light`), so there is no flash.
  Components use the tokens in `layout.css` (`foreground`, `--glass`,
  `--shadow`, ...), never raw white or black. Surfaces that stay dark in both
  themes (the blueprint paper) take `.theme-dark`.
- One shared 3D stage (`$lib/stage.svelte.ts`, `$lib/components/stage.svelte`)
  lives in the root layout at z-10 with a transparent canvas. Pages set its mode
  (hero, dock, hidden) and ship; UI that must sit above it uses z-20, anything
  behind it (the ship page's background type) sits at z-0. Never resize the canvas
  for a mode change; the dock is the same canvas scaled and clipped.
- From the blueprint chapter down the stage docks into the sheet's side
  elevation (`stage.blueprintFit` frames it at the drawing's scale) and stays
  there. The livery chapter shows its own view of the same ship instead (see
  the section views below), with two rails (`$lib/components/skin-rail.svelte`):
  the game's skins and the players' SKINR designs.
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
- Livery icons are the game's SKIN hexagon drawn as vectors
  (`$lib/components/skin-hexagon.svelte`, shapes and shading traced from the
  client's 64 px layers), so they stay sharp. In-game skins use the colors the
  import reads back off their icon (`src/sde/skin_icon_colors.rs`), SKINR
  designs their materials' swatch colors plus their patterns; the 64 px icon
  (`/api/skins/{material}.png`) is only the fallback.
  `$lib/components/skin-icon.svelte` picks.
- A resource read that fails is reported by the renderer (`onMissing`) and
  collected in `stage.missing`; a broken ship glitches (the same post pass,
  `setGlitch`) and `$lib/components/stage-alert.svelte` says why. Runtime
  formats are opt-in: register any new one in `resMan.Register({ formats })`.
- ESI goes through `src/esi/client.rs` only: it pins `COMPATIBILITY_DATE`
  (undated requests get the 2020 API, without SKINR), names the app and backs
  off on 420/429. Look routes up with the global `esi` skill, not from memory.
- SKINR (player designs): the API syncs Paragon Hub listings, designs and
  names (`src/esi/skinr.rs`, every 10 min, `ESI_SYNC=0` turns it off); the SDE
  import adds the components and the faction slot mapping. The frontend turns
  a design into SOF DNA (`material?…`) plus a synthetic SOF pattern for its
  projected layers (`$lib/skinr.ts`), registered with the engine before the
  build (`stage.pattern`, `engine.registerPattern`).
- Chapters with their own ship (the liveries) use
  `$lib/components/section-stage.svelte`; each section view has a key and its
  own canvas and camera in the renderer (`sectionCanvas`, `setSectionView`).
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
- Ship thumbnails (`<id>-dark.png`, `<id>-light.png`) are rendered without a
  browser: `deno run -A --node-modules-dir=manual scripts/render-thumbnails.ts`
  in `frontend/` with the API up (`API=http://localhost:5200` for docker). It
  writes `storage/thumbnails`, which the API serves at `/api/thumbnails`, and
  skips ships already done, so an interrupted run just starts again.
- Before committing: `cargo fmt`, `cargo clippy --all-targets` (zero warnings),
  `cargo test`, and in `frontend/` `npm run check`, `npm run lint`,
  `npm run format:check`, `npm test`.
