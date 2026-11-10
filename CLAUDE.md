# skinr.space

Try on EVE Online ship skins (the in-game ones and SKINR designs on sale)
on the real hull in 3D. A Rust API (Axum + Postgres via sqlx) serving JSON plus the game's files under `/cdn`, and a SvelteKit
frontend in `frontend/` (SSR, adapter-node, Tailwind v4, shadcn-svelte). The
setup mirrors MutaMarket (`../MutaMarket`): when in doubt about structure,
theme or tooling, do what it does.

## Rules

- One origin: the Vite dev proxy (`frontend/proxy-paths.ts`) and
  `deploy/Caddyfile` route `/api`, `/cdn`, `/eve` (the SSO), `/og` (link
  preview cards) and `/sitemap.xml` to Axum. Keep them in sync and never
  point the browser at Axum's port.
- Game files reach the browser by `res:/` path through the API
  (`src/game_files`): `/cdn/build`, `/cdn/res/{build}/{path}` (kept on disk,
  zstd where accepted) and `/cdn/list/{build}/{folder}`. The API holds CCP's
  file index; the browser never loads it. `#lib/viewer/resources.ts` is the
  only client, and preloads a picked ship's files (`/api/ships/{id}/files`).
- Ship data comes from the native SDE import (`cargo run --bin sde_import`),
  never hand-written fixtures in production.
- SDE text (descriptions and anything else from the game) is EVE HTML, not HTML.
  Store it raw; render it only through `src/eve_html.rs` (plain text, or styled
  segments the frontend draws with `#lib/components/eve-text.svelte`), never
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
- Themes: `#lib/theme` keeps the choice in a cookie that `hooks.server.ts`
  turns into the class on `<html>` (`dark` or `light`), so there is no flash.
  Components use the tokens in `layout.css` (`foreground`, `--glass`,
  `--shadow`, ...), never raw white or black. Surfaces that stay dark in both
  themes (the blueprint paper) take `.theme-dark`.
- One shared 3D stage (`#lib/stage.svelte.ts`, `#lib/components/stage.svelte`)
  lives in the root layout at z-10 with a transparent canvas. Pages declare a
  scene with `useScene` (its place: `hero`, `peek` or `blueprint`, plus pose,
  placement, dim) and pick the ship; they never write the stage's fields. UI
  that must sit above it uses z-20, anything behind it (the ship page's
  background type) sits at z-0. Never resize the canvas for a change of place;
  a dock is the same canvas scaled and clipped.
- Where the stage is and how it gets somewhere else is one state machine,
  `#lib/stage-machine.ts` (hidden, appearing, shown, gliding, holding,
  leaving, leaping), stepped every frame by the stage. Every change of place
  on screen glides; a stage out of sight shows only once its ship has loaded.
  Change behaviour there, with a test, not with flags in the component. Each
  state change and navigation goes to `stage.trace` (the console in dev and
  with the debug tools).
- From the blueprint chapter down the stage docks into the sheet's side
  elevation (the scene's `fit` frames it at the drawing's scale) and stays
  there. The livery chapter shows its own view of the same ship instead (see
  the section views below), with two rails (`#lib/components/skin-rail.svelte`):
  the game's skins and the players' SKINR designs.
- On the ship page the chapter alone picks the scene.
- URLs read: a ship is `/ships/{name-slug}` (an id redirects there), one of
  its skins `/ships/{ship}/skins/{skin-slug}`, a SKINR design
  `/ships/{ship}/skinr/{name-slug}-{first 12 of its id}`, a designer
  `/skinr/designer/{name-slug}-{id}` (ids decide). The ship page is one route
  (`ships/[ship]/[...livery]`): picking a livery changes the path, not the
  page. Build URLs with `shipHref`, `skinHref`, `designHref` and
  `designerHref` (`#lib/urls.ts`), never by hand; the sitemap's `src/urls.rs`
  must agree with them.
- Every page's head goes through `#lib/components/seo.svelte` (title,
  description, canonical, Open Graph/Twitter card from `/og/…`, JSON-LD).
  `PUBLIC_ORIGIN` makes its URLs absolute.
- Phones: below `lg` there is no preview panel (a card opens its ship), the
  livery chapter stacks its ship above the text, and grids of text that can
  run long need `min-w-0` (or `minmax(0, 1fr)` columns) so they cannot widen
  the page.
- The preview panel on the right (explorer, Paragon Hub, `/me`) is one
  component, `#lib/components/peek-panel.svelte`: the stage's dock, the
  page's labels over it, the resize handle and the footer clearance. Its
  width is `peek.css` (`#lib/peek.svelte.ts`, saved in a cookie) as
  `--peek`, which does not inherit: each element that uses it sets
  `style:--peek={peek.css}` itself (the page's main column for its padding),
  never an ancestor, or every frame of a drag restyles the whole page. Code
  that runs during the drag must not read the layout right after a change
  (no `getBoundingClientRect` in an effect on the width), and carousels skip
  their re-measure while `peek.resizing`. Pages only pass their labels and
  backdrop. The wheel zooms the stage in it, and needs ctrl wherever the
  page scrolls under the canvas.
- Page transitions: `#lib/page-transitions.ts` is the one table of how the
  page moves between two areas (sweep, slide the way the site bar reads,
  morph, fade); `+layout.svelte` runs it as a View Transition and `layout.css`
  has the animations. The site bar, the stage and a panel both pages have
  stay put. Shared elements morph by `view-transition-name`, one
  element per name and page.
- The site bar (`#lib/components/site-bar.svelte`) lives in the root layout;
  pages put their breadcrumb, section links and reset action into
  `#lib/nav.svelte.ts` while mounted and clear it on the way out.
- Write `backdrop-filter` without a hand-written `-webkit-` twin: the build's
  CSS minifier then keeps only the prefixed one, which Chrome ignores. It adds
  the prefix itself.
- The blueprint section's paper (`--blueprint-paper`) is a z-0 layer measured
  to the section, not the section's own background: page content sits at z-20,
  above the stage, and would hide the docked ship.
- Attribute icons on the sheet are the client's own (`/api/icons/{attribute}.png`,
  fetched by the SDE import); use `#lib/components/attribute-icon.svelte`.
- Livery icons are the game's SKIN hexagon drawn as vectors
  (`#lib/components/skin-hexagon.svelte`, shapes and shading traced from the
  client's 64 px layers), so they stay sharp. In-game skins use the colors the
  import reads back off their icon (`src/sde/skin_icon_colors.rs`), SKINR
  designs their materials' swatch colors plus their patterns; the 64 px icon
  (`/api/skins/{material}.png`) is only the fallback.
  `#lib/components/skin-icon.svelte` picks. Skin and SKINR design share
  cards carry the same badge, drawn by `src/og/hexagon.rs`, a port of
  `skin-hexagon.svelte`: change the two together.
- A resource read that fails is reported by the renderer (`onMissing`) and
  collected in `stage.missing`; a broken ship glitches (the same post pass,
  `setGlitch`) and `#lib/components/stage-alert.svelte` says why. The runtime
  is set up through `CjsLibrary` (its defaults cover PNG, textures, geometry
  and object files); add any other format to its `formats` configuration.
- ESI goes through `src/esi/client.rs` only: it pins `COMPATIBILITY_DATE`
  (undated requests get the 2020 API, without SKINR), names the app and backs
  off on 420/429. Look routes up with the global `esi` skill, not from memory.
- SKINR (player designs): the API syncs Paragon Hub listings, designs and
  names (`src/esi/skinr.rs`, every 10 min, `ESI_SYNC=0` turns it off); the SDE
  import adds the components and the faction slot mapping. The frontend turns
  a design into SOF DNA (`material?…`) plus a synthetic SOF pattern for its
  projected layers (`#lib/skinr.ts`), registered with the engine before the
  build (`stage.pattern`, `engine.registerPattern`).
- Log in with EVE (`src/auth/`): the API does the whole SSO round trip
  (`/eve/login`, `/eve/callback`, `POST /eve/logout`), checks
  the JWT against CCP's JWKS, keeps tokens in Postgres and refreshes them;
  the browser only ever holds the HttpOnly session cookie. The only scope is
  `esi.cosmetic.char:read`. Login is off until `EVE_CLIENT_ID`,
  `EVE_CLIENT_SECRET` and `EVE_CALLBACK_URL` are set. A character's ESI
  answers go through `src/esi/cache.rs`, which keeps them until they expire.
- The SKINR pages (`/skinr`, the hub; `/me`, a character's own) dock the
  stage in `#lib/components/skinr-dock.svelte` and keep the picked design in
  `?skinr=` (`#lib/skinr-stage.svelte.ts`).
- Rust tests that need Postgres get a database of their own
  (`tests/common/mod.rs`, `TEST_DATABASE_URL`).
- Chapters with their own ship (the liveries) use
  `#lib/components/section-stage.svelte`; each section view has a key and its
  own canvas and camera in the renderer (`sectionCanvas`, `setSectionView`).
- The fitting window look is our own WebGPU pass (`frontend/src/lib/viewer/hologram.ts`)
  over the finished frame, not a runtime wireframe mode (WebGPU has none).
- The spec sheet reads only the dogma attributes listed in `src/sheet.rs`
  (`SHEET_ATTRIBUTES`); a test fails if it reads one that is not imported.
- Only one element per page may carry `view-transition-name: ship-name`
  (`stage.morphId` picks it).
- `@carbonenginejs/runtime` is pre-alpha. Workarounds live in
  `frontend/src/lib/viewer/renderer.ts`, `frontend/src/lib/viewer/ship.ts`
  and `frontend/vite.config.ts`, each commented; remove them as the runtime
  fills the gaps.
- The blueprint draws the hull alone (`hullOnly` in `renderer.ts`): a
  livery's effects must not change its size, or the page jumps.
- WebGPU needs a headed Chromium on Linux; `frontend/scripts/shot.js` opens one.
- Open Graph cards live in `src/og` (as in MutaMarket): SVG set in the
  embedded Inter faces (`assets/fonts`), rasterized with resvg, served under
  `/og/...` and kept in `OG_CACHE_DIR`. A card's file name hashes everything
  it shows, so it redraws when that changes; bump `CARD_VERSION` after a
  layout change. Livery art comes from the render worker (`RENDERER_URL`);
  a card that cannot wait for it shows the ship thumbnail, is not stored and
  is cached briefly, while the render lands on disk for the next request.
- The render worker (`frontend/scripts/render-server.ts`, the `renderer`
  service) is Node with WebGPU from the `webgpu` package (Dawn), no Deno and
  no browser; its studio is `frontend/scripts/render/studio.ts`, and Node
  runs those files by stripping types, so shared viewer code must not use
  TypeScript-only syntax (parameter properties, enums). It is internal only,
  runs `WORKERS` warm child processes that each take one shot at a time and
  are replaced every `MAX_RENDERS`, and draws SKINR designs through
  `#lib/skinr.ts`. A shot is mostly one busy thread, so the pool is what
  uses the machine; compose sets `WORKERS` and the API's
  `THUMBNAIL_CONCURRENCY` from one `RENDER_WORKERS`. Themes with the same
  look share one picture. A shot is taken on the first frame that started
  with nothing loading and asked for nothing new, at a fixed engine clock
  and random seed, after `WARM_UP_UPDATES` scene updates (booster glow eases
  in per update, not per second), so a livery always gets the same picture.
  Production has no GPU: Mesa's lavapipe draws on the CPU (it has BC
  compression).
- Thumbnails belong to the API: `src/thumbnails.rs` asks the worker
  (`POST /thumbnail`, a DNA or a SKINR design) for every ship, then every
  SKINR design on sale, every in-game skin and the other designs, whose DNA (a
  design's look, `dna::design_look`) differs from its `.dna`, at start and
  hourly (`THUMBNAIL_SYNC=0` turns it off). Each gets `-dark.png`,
  `-light.png`, `.files` (the paths the render used, for preloading) and the
  `.dna` last in `THUMBNAILS_DIR`: `<ship>`, `skins/<ship>_<skin>` (a skin
  can fit several hulls), `designs/<design id>`, served at
  `/api/thumbnails/...` (and as WebP at card sizes, `-<theme>-{256,512}.webp`).
  Pictures of a livery go through `#lib/components/livery-thumbnail.svelte`
  (its own thumbnail, then the bare hull's, then CCP's icon); skin and design
  Open Graph cards show it while their own render is missing.
- The debug tools (`DEBUG_TOOLS=1` on the API and the frontend, on in docker
  compose, never in production) draw a livery's thumbnails on request:
  `src/server/debug.rs` under `/api/debug`, and the bug button by the ship
  page's livery names (`#lib/components/thumbnail-debug.svelte`), which also
  shows the page's share card.
- SOF DNA strings are built by `src/dna.rs` only.
- `/sitemap.xml` comes from the database. Page URLs are built in Rust by
  `src/urls.rs` and must match `#lib/urls.ts` exactly (same slug rules, same
  test cases); change both together.

## Commands

- `docker compose up --build` serves everything on http://localhost:5200.
- `docker compose watch` keeps it current: it rebuilds and replaces the api
  or frontend container when their code changes (`develop.watch` in the
  compose file). The API and the SDE import share one image, so a rebuild
  after a new migration updates both.
- `docker compose up -d postgres`, `cargo run`, then `npm run dev -- --port 5200`
  in `frontend/` for development (Postgres on 127.0.0.1:5434, API on 3100).
- Thumbnails outside docker: run the worker with
  `API=http://127.0.0.1:3100 node scripts/render-server.ts` in `frontend/`
  and the API with `RENDERER_URL=http://127.0.0.1:3200`; it fills
  `storage/thumbnails`. In docker they live in the `thumbnails` volume.
- Before committing: `cargo fmt`, `cargo clippy --all-targets` (zero warnings),
  `cargo test`, and in `frontend/` `npm run check`, `npm run lint`,
  `npm run format:check`, `npm test`.
