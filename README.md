# Ship Viewer

Browse every ship in EVE Online and view it in 3D in the browser, in any of
its liveries. Ships are built from the game's own data: the Static Data
Export (SDE) says which Space Object Factory hull, faction and race make up
each ship and skin, and [CarbonEngineJS](https://github.com/carbonenginejs/runtime)
builds and renders them with EVE's shaders on WebGPU.

A Rust JSON API (Axum, Postgres via sqlx) and a SvelteKit frontend, the same
setup as MutaMarket. Everything runs on one origin: Caddy (or the Vite dev
proxy) sends `/api` and `/cdn` to Axum and every page to SvelteKit.

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
- `deploy/Caddyfile.dev` the shared-origin routing in docker.

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
