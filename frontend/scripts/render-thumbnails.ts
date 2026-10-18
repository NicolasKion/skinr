// Renders a thumbnail of every ship, once for each theme, without a browser:
// Deno's own WebGPU, the runtime's WebGPU backend and EVE's shaders, read
// from CCP's CDNs. Run from frontend/ with the API up:
//
//   deno run -A --node-modules-dir=manual scripts/render-thumbnails.ts [ship id...]
//
// Writes <id>-dark.png and <id>-light.png (transparent, SIZE px square, from
// THUMBNAIL_POSE) into OUT, skipping ships that already have both, so a run
// that stops can simply be started again. Every CDN file is cached in CACHE.
//
// The runtime keeps everything it has loaded, so ships are rendered in
// batches, each in a fresh process that starts warm from the disk cache.
/* eslint-disable @typescript-eslint/no-explicit-any */

/** Where the ship list comes from. */
const API = Deno.env.get('API') ?? 'http://127.0.0.1:3100';
/** Where the thumbnails go (storage/ is not in git). */
const OUT = Deno.env.get('OUT') ?? '../storage/thumbnails';
/** Where CDN files are kept between runs. */
const CACHE = Deno.env.get('CACHE') ?? '../storage/cdn-cache';
/** Width and height of each thumbnail, in pixels. */
const SIZE = Number(Deno.env.get('SIZE') ?? 512);
/** Ships per render process: bounds how much one process holds on to. */
const BATCH = Number(Deno.env.get('BATCH') ?? 25);
/** The flag that makes a process render the ships it is given instead of planning. */
const CHILD = '--child';

const THEMES = ['dark', 'light'] as const;
/**
 * Overrides the studio look (lighting.ts) per theme, as JSON, for trying
 * others: {"dark": {...}, "light": {...}}.
 */
const LOOKS_OVERRIDE = Deno.env.get('LOOKS');
const file = (id: number, theme: string) => `${OUT}/${id}-${theme}.png`;
const exists = (path: string) =>
  Deno.stat(path).then(
    () => true,
    () => false,
  );

if (Deno.args[0] === CHILD) {
  await renderShips(Deno.args.slice(1).map((arg) => arg.split('=') as [string, string]));
} else {
  await plan(Deno.args.map(Number));
}

/** Finds the ships still missing a thumbnail and renders them, a batch per process. */
async function plan(only: number[]) {
  const catalog = await (await fetch(`${API}/api/ships`)).json();
  const ships: { id: number; name: string; dna: string }[] = catalog.ships.filter(
    (ship: any) => ship.dna && (!only.length || only.includes(ship.id)),
  );
  await Deno.mkdir(OUT, { recursive: true });
  const todo = [];
  for (const ship of ships) {
    // Cheap checks, one after another, keep the plan readable.
    // oxlint-disable-next-line no-await-in-loop
    const done = await Promise.all(THEMES.map((theme) => exists(file(ship.id, theme))));
    if (!done.every(Boolean)) todo.push(ship);
  }
  console.log(
    `${ships.length} ships, ${ships.length - todo.length} done, ${todo.length} to render`,
  );

  const started = performance.now();
  for (let at = 0; at < todo.length; at += BATCH) {
    const batch = todo.slice(at, at + BATCH);
    const child = new Deno.Command(Deno.execPath(), {
      args: [
        'run',
        '-A',
        '--node-modules-dir=manual',
        new URL(import.meta.url).pathname,
        CHILD,
        ...batch.map((ship) => `${ship.id}=${ship.dna}`),
      ],
      stdout: 'inherit',
      stderr: 'inherit',
    });
    // Batches run one at a time: they share the GPU.
    // oxlint-disable-next-line no-await-in-loop
    const { code } = await child.output();
    const seconds = (performance.now() - started) / 1000;
    const count = Math.min(at + BATCH, todo.length);
    console.log(
      `${count}/${todo.length} in ${seconds.toFixed(0)} s` +
        (code ? ` (the process for this batch exited with ${code})` : ''),
    );
  }
}

/** Renders each `[id, dna]` in both themes: the child process's work. */
async function renderShips(ships: [string, string][]) {
  const { blue } = await import('@carbonenginejs/runtime/blue');
  const resource = await import('@carbonenginejs/runtime/resource');
  const { RegisterObjectResources } = await import('@carbonenginejs/runtime/resource/object');
  const { CjsWebgpuFormat } = await import('@carbonenginejs/runtime/resource/formats/webgpu');
  const { CjsPngFormat } = await import('@carbonenginejs/runtime/resource/formats/png');
  const { ImageIO } = await import('@carbonenginejs/runtime/resource/imageio');
  const { SetEffectPathDefaults } = await import('@carbonenginejs/runtime/utils');
  const { CjsSchema } = await import('@carbonenginejs/runtime/schema');
  await import('@carbonenginejs/runtime/trinity');
  // Ships carry sound emitters; the audio classes must exist to hydrate them.
  await import('@carbonenginejs/runtime/audio/trinity');
  const core = await import('@carbonenginejs/runtime/trinity/core');
  const { EveSpaceScene } = await import('@carbonenginejs/runtime/trinity/eve');
  const { EveSpaceSceneRenderDriver } = await import('@carbonenginejs/runtime/trinity');
  const { CjsWebgpuDevice } = await import('@carbonenginejs/runtime/trinityal/webgpu');
  // Not in the runtime's package exports yet (see vite.config.ts); the path
  // resolves to the same module the public webgpu entry uses.
  const { CjsWebgpuRenderContextAL, CjsWebgpuRenderTarget } =
    await import('../node_modules/@carbonenginejs/runtime/dist/trinityal/webgpu/internal.js');
  const { CjsFileIndexLibrary } = await import('@carbonenginejs/runtime/tools/fileindex');
  const { EveSOF } = await import('@carbonenginejs/runtime/sof');
  const { encodePng } = await import('../src/lib/viewer/png.ts');
  const { placement } = await import('../src/lib/viewer/placement.ts');
  const { alignBlockTextures } = await import('../src/lib/viewer/block-align.ts');
  const { STUDIO_LOOK, applyLighting, placeSun, takeLighting } =
    await import('../src/lib/viewer/lighting.ts');
  const looks = LOOKS_OVERRIDE
    ? JSON.parse(LOOKS_OVERRIDE)
    : { dark: STUDIO_LOOK, light: STUDIO_LOOK };
  const { FIELD_OF_VIEW, THUMBNAIL_FRAMING, THUMBNAIL_POSE } =
    await import('../src/lib/viewer/poses.ts');

  /** The site's universe scene: sun, ambient and reflection maps. */
  const SCENE = 'res:/dx9/scene/universe/c01_cube.red';
  const { margin: MARGIN, padding: PADDING, solid: SOLID } = THUMBNAIL_FRAMING;
  /** Frames the resource manager must stay idle before a picture counts. */
  const QUIET_FRAMES = 10;
  /** Gives up on a ship that is still loading after this many frames. */
  const MAX_FRAMES = 3000;
  const FORMAT = 'rgba8unorm';

  /** Fetches a CDN file, from the disk cache when it is there. */
  async function cached(url: string): Promise<Uint8Array> {
    const path = `${CACHE}/${new URL(url).host}${new URL(url).pathname}`;
    try {
      return await Deno.readFile(path);
    } catch {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`HTTP ${response.status} for ${url}`);
      const bytes = new Uint8Array(await response.arrayBuffer());
      await Deno.mkdir(path.slice(0, path.lastIndexOf('/')), { recursive: true });
      // Written aside and moved, so a killed run never leaves half a file.
      await Deno.writeFile(`${path}.part`, bytes);
      await Deno.rename(`${path}.part`, path);
      return bytes;
    }
  }

  // The game client's file index and SOF, straight from CCP (no CORS outside a browser).
  const library = await CjsFileIndexLibrary.load(
    {
      game: 'Eve',
      id: 'ccp',
      defaultBuildRef: 'latest',
      remote: {
        metadataBaseURL: 'https://binaries.eveonline.com',
        indexBaseURL: 'https://binaries.eveonline.com',
        appBaseURL: 'https://binaries.eveonline.com',
        resBaseURL: 'https://resources.eveonline.com',
      },
      clients: { tranquility: { metadataToken: 'TQ', aliases: ['tq'] } },
    },
    { build: 'latest' },
  );
  const resolve = (path: string): string | null =>
    (
      library.Resolve(path) ??
      (path.endsWith('.red') ? library.Resolve(`${path.slice(0, -4)}.black`) : null)
    )?.sourceURL ?? null;
  /** res:/ paths the current ship could not read. */
  let missing: string[] = [];
  const read = async (path: string) => {
    // Shaders keep their webgpu path; the bytes behind it are the shipped dx11 effect.
    const url = resolve(path.replace('/effect.webgpu/', '/effect.dx11/'));
    try {
      if (!url) throw new Error(`Not in the file index: ${path}`);
      return await cached(url);
    } catch (error) {
      missing.push(path);
      throw error;
    }
  };

  const sof = await new EveSOF().Register({
    lazyData: {
      source: async (path: string) => {
        const url = resolve(path);
        if (!url) throw new Error(`Not in the file index: ${path}`);
        return new Response(await cached(url));
      },
    },
    resources: { exists: async (path: string) => !!library.Resolve(path) },
  });
  await sof.InitializeAsync();

  blue.resMan.Register({
    source: { Read: read },
    formats: [CjsPngFormat],
    useWorkerLoading: false,
  });
  ImageIO.registerImageHandler(CjsPngFormat);
  resource.RegisterGeometryResources(blue.resMan);
  resource.RegisterTextureResources(blue.resMan);
  resource.RegisterTextureArray(blue.resMan);
  resource.RegisterTexturePack(blue.resMan);
  resource.RegisterSolidColorTexture(blue.resMan);
  RegisterObjectResources(blue.resMan);
  resource.RegisterShaderResources(blue.resMan, {
    translator: CjsWebgpuFormat,
    permutations: 'onDemand',
  });
  SetEffectPathDefaults({ platformName: 'webgpu', shaderModel: 'high' });

  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) throw new Error('No WebGPU adapter.');
  const requiredFeatures = [
    'texture-compression-bc',
    'texture-compression-bc-sliced-3d',
    'float32-filterable',
    'depth32float-stencil8',
    'rg11b10ufloat-renderable',
  ].filter((feature) => adapter.features.has(feature as GPUFeatureName));
  const webgpu = await CjsWebgpuDevice.Request({ adapter, deviceDescriptor: { requiredFeatures } });
  const device: GPUDevice = webgpu.GetDevice();
  alignBlockTextures(device);
  /** WebGPU errors do not throw; a frame that hits one is dropped whole. */
  let gpuErrors: string[] = [];
  device.addEventListener('uncapturederror', (event: any) => {
    gpuErrors.push(event.error?.message ?? String(event.error));
  });

  // The runtime draws into a canvas context; this stand-in hands it a plain
  // texture instead, which is copied back once a picture is done.
  let frameTexture: GPUTexture | null = null;
  const context = {
    configure(descriptor: GPUCanvasConfiguration) {
      frameTexture?.destroy();
      frameTexture = device.createTexture({
        size: [SIZE, SIZE],
        format: descriptor.format,
        usage: (descriptor.usage ?? 0) | GPUTextureUsage.COPY_SRC,
      });
    },
    unconfigure() {},
    getCurrentTexture: () => frameTexture,
  };
  const target = new CjsWebgpuRenderTarget(webgpu, {
    context,
    format: FORMAT,
    depthFormat: 'depth32float',
    alphaMode: 'premultiplied',
  });
  // Clear depth (reverse-Z far is 0) and colour on each frame's first pass,
  // as renderer.ts does for the page.
  const create = target.CreateRenderPassDescriptor.bind(target);
  let lastFrame: unknown = null;
  target.CreateRenderPassDescriptor = (frame: unknown, options: any = {}) => {
    if (frame !== lastFrame) {
      options = { ...options, clearDepth: 0, clearColor: { r: 0, g: 0, b: 0, a: 0 } };
    }
    lastFrame = frame;
    return create(frame, options);
  };
  target.Configure({ width: SIZE, height: SIZE });
  const renderContext = core.Tr2RenderContext_GetMainThreadRenderContext();
  renderContext.SetRenderContextAL(new CjsWebgpuRenderContextAL({ webgpu, renderTarget: target }));

  const tri = new core.TriDevice();
  const canvas = { width: SIZE, height: SIZE, clientWidth: SIZE, clientHeight: SIZE };
  if (!tri.CreateSimpleDevice(canvas, SIZE, SIZE)) throw new Error('No Trinity device.');

  const loaded = await blue.resMan.LoadObject(SCENE).catch(() => null);
  const scene = loaded instanceof EveSpaceScene ? loaded : new EveSpaceScene();
  const lighting = takeLighting(scene);
  const view = new core.TriView();
  const projection = new core.TriProjection();
  const driver = new EveSpaceSceneRenderDriver();
  driver.scene = scene;
  driver.view = view;
  driver.projection = projection;
  driver.clearColor = [0, 0, 0, 0];
  driver.SetBatchManager(new core.CjsBatchManager().Initialize());
  tri.SetRenderJobs({
    RunUpdate: () => {},
    Run: (realTime: number, simTime: number) =>
      driver.Execute(null, null, realTime, simTime, null, renderContext),
  });
  placeSun(scene, THUMBNAIL_POSE.yaw, THUMBNAIL_POSE.pitch);

  /** Draws frames until the resource manager has stayed idle a while; false on a timeout. */
  async function settle() {
    let quiet = 0;
    for (let frame = 0; frame < MAX_FRAMES; frame++) {
      blue.os.PumpOS();
      // One frame at a time is the point: each waits on the last.
      // oxlint-disable-next-line no-await-in-loop
      await device.queue.onSubmittedWorkDone();
      quiet = blue.resMan.IsLoading() ? 0 : quiet + 1;
      if (quiet >= QUIET_FRAMES) return true;
      // Let fetches and decodes run between frames.
      // oxlint-disable-next-line no-await-in-loop
      await new Promise((done) => setTimeout(done, 0));
    }
    return false;
  }

  /**
   * Settles the frame, and fails the ship when a GPU error dropped frames:
   * the texture would still hold an older picture.
   */
  async function ready() {
    if (!(await settle())) throw new Error('still loading after the frame limit');
    if (gpuErrors.length) throw new Error(`GPU error: ${gpuErrors[0].slice(0, 200)}`);
  }

  /** Copies the last frame back as straight-alpha RGBA. */
  async function readFrame() {
    // Rows of a texture copy are padded to 256 bytes.
    const bytesPerRow = Math.ceil((SIZE * 4) / 256) * 256;
    const buffer = device.createBuffer({
      size: bytesPerRow * SIZE,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const encoder = device.createCommandEncoder();
    encoder.copyTextureToBuffer({ texture: frameTexture! }, { buffer, bytesPerRow }, [SIZE, SIZE]);
    device.queue.submit([encoder.finish()]);
    await buffer.mapAsync(GPUMapMode.READ);
    const padded = new Uint8Array(buffer.getMappedRange());
    const pixels = new Uint8ClampedArray(SIZE * SIZE * 4);
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        const from = y * bytesPerRow + x * 4;
        const to = (y * SIZE + x) * 4;
        // The frame holds premultiplied color; PNG wants it straight.
        const alpha = padded[from + 3];
        const unpremultiply = alpha ? 255 / alpha : 0;
        pixels[to] = padded[from] * unpremultiply;
        pixels[to + 1] = padded[from + 1] * unpremultiply;
        pixels[to + 2] = padded[from + 2] * unpremultiply;
        pixels[to + 3] = alpha;
      }
    }
    buffer.unmap();
    buffer.destroy();
    return pixels;
  }

  /** The box around the hull's pixels, or null when nothing was drawn. */
  function hullBox(pixels: Uint8ClampedArray) {
    let [left, top, right, bottom] = [SIZE, SIZE, -1, -1];
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        if (pixels[(y * SIZE + x) * 4 + 3] < SOLID) continue;
        left = Math.min(left, x);
        right = Math.max(right, x);
        top = Math.min(top, y);
        bottom = Math.max(bottom, y);
      }
    }
    return right < 0 ? null : { left, top, right: right + 1, bottom: bottom + 1 };
  }

  async function save(path: string, pixels: Uint8ClampedArray) {
    const png = await encodePng(pixels, SIZE, 0, 0, SIZE, SIZE);
    // Written aside and moved, so a killed run never leaves a broken thumbnail.
    await Deno.writeFile(`${path}.part`, new Uint8Array(await png.arrayBuffer()));
    await Deno.rename(`${path}.part`, path);
  }

  /** Builds one ship, frames it, and writes it in both themes. */
  async function renderShip(id: number, dna: string) {
    const started = performance.now();
    missing = [];
    gpuErrors = [];
    const values = await sof.BuildValuesFromDNAAsync(dna);
    const ship = CjsSchema.from('EveShip2', values);
    ship.translationCurve = placement([0, 0, 0]);
    scene.objects.push(ship);
    try {
      // The whole bounding sphere in view, whatever the ship's shape.
      const center = values.boundingSphereCenter ?? [0, 0, 0];
      const radius = values.boundingSphereRadius ?? 50;
      const distance = (radius * MARGIN) / Math.sin(FIELD_OF_VIEW / 2);
      const { yaw, pitch } = THUMBNAIL_POSE;
      const c = Math.cos(pitch);
      const eye = [
        center[0] + distance * c * Math.sin(yaw),
        center[1] + distance * Math.sin(pitch),
        center[2] + distance * c * Math.cos(yaw),
      ];
      const up = [
        -Math.sin(pitch) * Math.sin(yaw),
        Math.cos(pitch),
        -Math.sin(pitch) * Math.cos(yaw),
      ];
      view.SetLookAtPosition(eye, center, up);
      const near = Math.max(0.1, distance / 100);
      const far = distance * 20;
      projection.PerspectiveFov(FIELD_OF_VIEW, 1, near, far);

      // A first look, to find where the hull actually is on screen.
      applyLighting(scene, lighting, looks.dark);
      await ready();
      const box = hullBox(await readFrame());
      if (!box) throw new Error('nothing was drawn');

      // Narrow the view onto that box, made square and padded: the same
      // camera, angle and perspective, only cropped to the hull.
      const half = near * Math.tan(FIELD_OF_VIEW / 2);
      const toPlane = (pixel: number) => (pixel / SIZE) * 2 * half - half;
      const centerX = toPlane((box.left + box.right) / 2);
      const centerY = -toPlane((box.top + box.bottom) / 2);
      const extent = (Math.max(box.right - box.left, box.bottom - box.top) / SIZE) * half * PADDING;
      projection.PerspectiveOffCenter(
        centerX - extent,
        centerX + extent,
        centerY - extent,
        centerY + extent,
        near,
        far,
      );

      for (const theme of THEMES) {
        applyLighting(scene, lighting, looks[theme]);
        // Everything is loaded by now; this only lets a few frames through.
        // oxlint-disable-next-line no-await-in-loop
        await ready();
        // oxlint-disable-next-line no-await-in-loop
        await save(file(id, theme), await readFrame());
      }
      const note = missing.length
        ? `, ${missing.length} missing: ${missing.slice(0, 3).join(' ')}`
        : '';
      console.log(`${id} ${dna}: ${Math.round(performance.now() - started)} ms${note}`);
    } finally {
      scene.objects.splice(scene.objects.indexOf(ship), 1);
    }
  }

  for (const [id, dna] of ships) {
    try {
      // Ships share the scene, so one at a time.
      // oxlint-disable-next-line no-await-in-loop
      await renderShip(Number(id), dna);
    } catch (error) {
      console.error(`${id} ${dna}: failed: ${(error as Error).message}`);
    }
  }
  Deno.exit(0);
}
