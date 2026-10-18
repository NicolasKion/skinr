# skinr.space

[![CI](https://github.com/NicolasKion/skinr/actions/workflows/ci.yml/badge.svg)](https://github.com/NicolasKion/skinr/actions/workflows/ci.yml)
[![Release](https://github.com/NicolasKion/skinr/actions/workflows/release.yml/badge.svg)](https://github.com/NicolasKion/skinr/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Axum-DEA584?logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-Svelte%205-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/docs/kit)
[![WebGPU](https://img.shields.io/badge/WebGPU-CarbonEngineJS-005A9C)](https://github.com/carbonenginejs/runtime)

Try on EVE Online ship skins on the real hull in 3D, right in the browser:
the in-game ones and the SKINR designs capsuleers have up for sale on the
Paragon Hub.

> [!NOTE]
> This is a rough first version and mostly vibe coded. It works, but it is
> an experiment, not a finished product: expect rough edges, missing
> pieces and things that change or break.

## Features

- Every ship in the game, grouped by class and by who builds it (the empires
  and the pirate and faction builders), straight from the SDE
- Each hull rendered with EVE's own shaders on WebGPU by
  [CarbonEngineJS](https://github.com/carbonenginejs/runtime), with every
  skin to try on it
- SKINR designs on sale, synced from ESI every 10 minutes, with their price
- A blueprint sheet per ship (side, top and front elevations) and its stats
- Related ships to jump to, with the 3D ship popping out of its card

## Stack

- API: Rust, [Axum](https://github.com/tokio-rs/axum) and
  [sqlx](https://github.com/launchbadge/sqlx) on Postgres. Ship and skin data
  come from CCP's Static Data Export, SKINR listings from ESI, and game files
  through a `/cdn` passthrough to CCP's client CDNs.
- Frontend: [SvelteKit](https://svelte.dev/docs/kit) (adapter-node), Svelte 5
  and Tailwind CSS 4

One origin serves both: Caddy (or the Vite dev proxy) sends `/api` and `/cdn`
to Axum and every page to SvelteKit.

## Run it

```sh
docker compose up --build       # http://localhost:5200
```

It starts Postgres, downloads and seeds the latest SDE when it changed, and
serves the app. Viewing ships needs a browser with WebGPU and BC texture
compression (desktop Chrome or Chromium; on Linux you may need
`--enable-unsafe-webgpu`).

For development, run Postgres in docker and the rest on the host:

```sh
docker compose up -d postgres
cp .env.example .env
cargo run --bin sde_import      # once, and after each SDE release
cargo run                       # API on 127.0.0.1:3100
cd frontend && npm install && npm run dev -- --port 5200
```

## Layout

- `src/sde/` the SDE import: download, parse ships, groups, races and skins, seed Postgres.
- `src/server/` the axum router and the JSON API (`/api/ships`, `/api/ships/{id}`, `/api/health`).
- `src/cdn.rs` the `/cdn/binaries` and `/cdn/resources` passthrough to CCP's client CDNs.
  The browser runtime reads game files through it, because CCP's CloudFront serves
  cached copies without CORS headers to browsers.
- `migrations/` the Postgres schema.
- `frontend/` the SvelteKit app (Tailwind v4, shadcn-svelte tooling, an Apple-style dark
  design). `src/lib/viewer/` holds the WebGPU renderer and the shared engine;
  `src/lib/components/stage.svelte` is the persistent 3D stage that moves between the
  ship hero and the explorer's dock.
- `deploy/Caddyfile` the shared-origin routing, local and in production.

## Deploy

Production runs on Coolify from `compose.prod.yml`. Every merge to main
builds the api, frontend and proxy images, pushes them to GHCR and calls the
Coolify deploy webhook (`COOLIFY_WEBHOOK` and `COOLIFY_TOKEN` repo secrets).
The SDE import runs before the api on each deploy. Thumbnails are rendered
locally and synced to the `skinr_thumbnails` volume (see the top of
`compose.prod.yml`).

## Checks

```sh
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
cd frontend && npm run check && npm run lint && npm run format:check && npm test
```

`node frontend/scripts/shot.js /ships/603 shot.png wait` screenshots a page of the
running app in Chromium. It opens a window, since headless Chromium usually has no
WebGPU adapter on Linux.

## Runtime workarounds

The runtime is pre-alpha. `frontend/src/lib/viewer/renderer.ts` and
`frontend/vite.config.ts` fill a few gaps, each commented in place:

- The composed WebGPU render context is not exported; a Vite plugin resolves it.
- Class files are missing from the package's `sideEffects` list, so production
  builds would drop ship classes; a Vite plugin keeps every runtime module.
- The canvas cannot be attached to a `Tr2RenderTarget`, so the scene draws straight
  into the canvas pass, without post-processing or the nebula background.
- On that path depth is never cleared; the canvas target clears it every frame.
- Re-creating the device on resize double-frees a pooled texture, so resizes only
  reconfigure the surface.
- The driver needs a `CjsBatchManager`, and `.red` paths are read as `.black`.

Lighting is still flat: the runtime does not drive sun lighting, shadows or
post-processing yet.

EVE Online and all related assets are the property of CCP hf. This project is not
affiliated with or endorsed by CCP.

## License

MIT, see [LICENSE](LICENSE). The EVE Online assets the app loads at runtime
are not covered by it.
