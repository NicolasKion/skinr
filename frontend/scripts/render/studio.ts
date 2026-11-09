// A headless studio that photographs ships: WebGPU in Node (Dawn, from the
// `webgpu` package), the runtime's WebGPU backend and EVE's shaders, read
// from CCP's CDNs (no CORS outside a browser) through a disk cache. The
// render worker (scripts/render-server.ts) draws every thumbnail and card
// with it, so they all look the same.
//
// The runtime keeps everything it has loaded and holds global state, so a
// process opens one studio and renders one ship at a time.
/* eslint-disable @typescript-eslint/no-explicit-any */

import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { create as createGpu, globals } from "webgpu";
import type { Look } from "../../src/lib/viewer/lighting.ts";

export const THEMES = ["dark", "light"] as const;
export type Theme = (typeof THEMES)[number];

export interface StudioOptions {
  /** Where CDN files are kept between runs (the API's game file cache shares the layout). */
  cache: string;
  /** Width and height of a picture, in pixels, unless a render asks for another. */
  size: number;
  /** Overrides the studio look per theme. */
  looks?: Partial<Record<Theme, Look>>;
}

/** One ship photographed. */
export interface Shot {
  /** A transparent PNG per theme asked for. */
  pngs: Partial<Record<Theme, Uint8Array>>;
  /** Every res:/ path the ship used, minus those that failed, sorted. */
  used: string[];
  /** res:/ paths the ship could not read. */
  missing: string[];
}

export interface Studio {
  /**
   * Builds a ship from its SOF DNA, frames it like a thumbnail
   * (THUMBNAIL_POSE, THUMBNAIL_FRAMING) and photographs it in `themes`.
   * Throws when nothing was drawn, the scene never settled or the GPU failed.
   */
  render(dna: string, themes: readonly Theme[], size?: number): Promise<Shot>;
  /** Makes a pattern the SOF does not ship (a SKINR design's) available to DNA under `name`. */
  registerPattern(name: string, record: object): void;
}

/** The site's universe scene: sun, ambient and reflection maps. */
const SCENE = "res:/dx9/scene/universe/c01_cube.red";
/** How often Blue is pumped while the scene loads. */
const PUMP_INTERVAL_MS = 16;
/**
 * The engine time every frame is drawn at, in seconds. Blinking lights,
 * turning parts and scrolling textures follow the clock; one fixed moment
 * gives each livery the same picture on every draw.
 */
const FIXED_TIME = 0;
/**
 * The seed the engine's random numbers restart from for each ship: booster
 * flicker phases and other effects draw from it as the ship is built.
 */
const RANDOM_SEED = 1;
/**
 * Scene updates run once a ship has loaded, before its picture. Booster
 * glow eases toward its level by a fifth per update rather than by time;
 * after these it is there in full (0.8^40 is under 0.01%).
 */
const WARM_UP_UPDATES = 40;
/** Gives up on a ship that is still loading after this many frames. */
const MAX_FRAMES = 3000;
const FORMAT = "rgba8unorm";
/** The feature the runtime cannot draw EVE's textures without. */
const REQUIRED_FEATURE = "texture-compression-bc";
/** Features the runtime uses when the adapter has them. */
const OPTIONAL_FEATURES = [
  "texture-compression-bc-sliced-3d",
  "float32-filterable",
  "depth32float-stencil8",
  "rg11b10ufloat-renderable",
];

/** Writes bytes aside and moves them into place, so a killed run never leaves half a file. */
async function writeAtomic(path: string, bytes: Uint8Array) {
  const part = `${path}.${process.pid}.part`;
  await writeFile(part, bytes);
  await rename(part, path);
}

/**
 * WebGPU for Node: the constants the browser has as globals
 * (GPUTextureUsage, ...) and `navigator.gpu`, which the runtime reads.
 */
function installWebgpu(): GPU {
  Object.assign(globalThis, globals);
  const gpu = createGpu([]);
  Object.defineProperty(navigator, "gpu", { value: gpu, configurable: true });
  return gpu;
}

/** Fetches a CDN file, from the disk cache when it is there. */
async function cached(cache: string, url: string): Promise<Uint8Array> {
  const { host, pathname } = new URL(url);
  const path = `${cache}/${host}${pathname}`;
  try {
    return new Uint8Array(await readFile(path));
  } catch {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`HTTP ${response.status} for ${url}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    await mkdir(dirname(path), { recursive: true });
    await writeAtomic(path, bytes);
    return bytes;
  }
}

/** Opens the game's file index and SOF, the runtime and a WebGPU device. */
export async function openStudio(options: StudioOptions): Promise<Studio> {
  const { cache } = options;
  const { blue } = await import("@carbonenginejs/runtime/blue");
  const { CjsLibrary } = await import("@carbonenginejs/runtime/core");
  const { CjsWebgpuFormat } =
    await import("@carbonenginejs/runtime/resource/formats/webgpu");
  await import("@carbonenginejs/runtime/trinity");
  // Ships carry sound emitters; the audio classes must exist to hydrate them.
  await import("@carbonenginejs/runtime/audio/trinity");
  const core = await import("@carbonenginejs/runtime/trinity/core");
  const { EveSpaceScene } = await import("@carbonenginejs/runtime/trinity/eve");
  const { EveSpaceSceneRenderDriver } =
    await import("@carbonenginejs/runtime/trinity");
  const { CjsWebgpuDevice } =
    await import("@carbonenginejs/runtime/trinityal/webgpu");
  // Not in the runtime's package exports yet (see vite.config.ts); the path
  // resolves to the same module the public webgpu entry uses.
  const { CjsWebgpuRenderContextAL, CjsWebgpuRenderTarget } =
    await import("../../node_modules/@carbonenginejs/runtime/dist/trinityal/webgpu/internal.js");
  const { CjsFileIndexLibrary } =
    await import("@carbonenginejs/runtime/tools/fileindex");
  const { EveSOF } = await import("@carbonenginejs/runtime/sof");
  const { random } = (await import("@carbonenginejs/runtime/math")).random;
  const { encodePng } = await import("../../src/lib/viewer/png.ts");
  const { placement } = await import("../../src/lib/viewer/placement.ts");
  const { buildShip } = await import("../../src/lib/viewer/ship.ts");
  const { alignBlockTextures } =
    await import("../../src/lib/viewer/block-align.ts");
  const { STUDIO_LOOK, applyLighting, placeSun, takeLighting } =
    await import("../../src/lib/viewer/lighting.ts");
  const { FIELD_OF_VIEW, THUMBNAIL_FRAMING, THUMBNAIL_POSE } =
    await import("../../src/lib/viewer/poses.ts");
  const looks: Record<Theme, Look> = {
    dark: options.looks?.dark ?? STUDIO_LOOK,
    light: options.looks?.light ?? STUDIO_LOOK,
  };
  const { margin: MARGIN, padding: PADDING, solid: SOLID } = THUMBNAIL_FRAMING;

  // The game client's file index and SOF, straight from CCP.
  const library = await CjsFileIndexLibrary.load(
    {
      game: "Eve",
      id: "ccp",
      defaultBuildRef: "latest",
      remote: {
        metadataBaseURL: "https://binaries.eveonline.com",
        indexBaseURL: "https://binaries.eveonline.com",
        appBaseURL: "https://binaries.eveonline.com",
        resBaseURL: "https://resources.eveonline.com",
      },
      clients: { tranquility: { metadataToken: "TQ", aliases: ["tq"] } },
    },
    { build: "latest" },
  );
  const resolve = (path: string): string | null =>
    (
      library.Resolve(path) ??
      (path.endsWith(".red")
        ? library.Resolve(`${path.slice(0, -4)}.black`)
        : null)
    )?.sourceURL ?? null;
  /** res:/ paths the current ship could not read. */
  let missing: string[] = [];
  /**
   * res:/ paths the current ship used: those it read, and those it looked up
   * (files an earlier ship loaded are not read again). Neither alone has
   * them all; some lookups bypass the hooked methods below.
   */
  const used = new Set<string>();
  /** Files read and resources looked up for the first time, ever growing. */
  let requests = 0;
  const read = async (path: string) => {
    const url = resolve(path);
    try {
      if (!url) throw new Error(`Not in the file index: ${path}`);
      requests += 1;
      const bytes = await cached(cache, url);
      used.add(path.toLowerCase());
      return bytes;
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
        return new Response((await cached(cache, url)) as BodyInit);
      },
    },
    resources: { exists: async (path: string) => !!library.Resolve(path) },
  });
  await sof.InitializeAsync();

  // The library's defaults register every format and resource type a ship
  // needs; the client's dx11 effects are translated to WebGPU on first use.
  const runtime = await new CjsLibrary({
    resMan: { source: { Read: read }, useWorkerLoading: false },
    callbackMan: { useWorkers: false },
    effects: {
      source: "dx11",
      target: "webgpu",
      translator: CjsWebgpuFormat,
      permutations: "onDemand",
      shaderModel: "high",
    },
  }).Initialize({ sof, characterLibrary: null });

  const adapter = await installWebgpu().requestAdapter({
    powerPreference: "high-performance",
  });
  if (!adapter) throw new Error("No WebGPU adapter.");
  if (!adapter.features.has(REQUIRED_FEATURE)) {
    throw new Error(`The WebGPU adapter lacks ${REQUIRED_FEATURE}.`);
  }
  const requiredFeatures = [REQUIRED_FEATURE, ...OPTIONAL_FEATURES].filter(
    (feature) => adapter.features.has(feature as GPUFeatureName),
  );
  const webgpu = await CjsWebgpuDevice.Request({
    adapter,
    deviceDescriptor: { requiredFeatures },
  });
  const device: GPUDevice = webgpu.GetDevice();
  alignBlockTextures(device);
  /** WebGPU errors do not throw; a frame that hits one is dropped whole. */
  let gpuErrors: string[] = [];
  device.addEventListener("uncapturederror", (event: any) => {
    gpuErrors.push(event.error?.message ?? String(event.error));
  });

  /** The picture size the device and frame texture have now. */
  let size = options.size;
  // The runtime draws into a canvas context; this stand-in hands it a plain
  // texture instead, which is copied back once a picture is done.
  let frameTexture: GPUTexture | null = null;
  const context = {
    configure(descriptor: GPUCanvasConfiguration) {
      frameTexture?.destroy();
      frameTexture = device.createTexture({
        size: [size, size],
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
    depthFormat: "depth32float",
    alphaMode: "premultiplied",
  });
  // Clear depth (reverse-Z far is 0) and colour on each frame's first pass,
  // as renderer.ts does for the page.
  const create = target.CreateRenderPassDescriptor.bind(target);
  let lastFrame: unknown = null;
  target.CreateRenderPassDescriptor = (
    frame: unknown,
    passOptions: any = {},
  ) => {
    if (frame !== lastFrame) {
      passOptions = {
        ...passOptions,
        clearDepth: 0,
        clearColor: { r: 0, g: 0, b: 0, a: 0 },
      };
    }
    lastFrame = frame;
    return create(frame, passOptions);
  };
  target.Configure({ width: size, height: size });
  const renderContext = core.Tr2RenderContext_GetMainThreadRenderContext();
  // Resources prepare on trinity.device, and PumpOS drives its render jobs.
  const tri = runtime.AttachTrinity({
    renderContextAL: new CjsWebgpuRenderContextAL({
      webgpu,
      renderTarget: target,
    }),
  });
  const canvas = {
    width: size,
    height: size,
    clientWidth: size,
    clientHeight: size,
  };
  if (!tri.CreateSimpleDevice(canvas, size, size))
    throw new Error("No Trinity device.");
  // Effects that animate on the device's own clock (booster flicker) stay put.
  tri.animationTime = FIXED_TIME;
  tri.animationTimeScale = 0;

  /**
   * Changes the picture size. Re-creating the device trips a double free in
   * the runtime's GPU resource pool (renderer.ts resize), so only the frame
   * texture and the viewport change.
   */
  function resize(next: number) {
    if (next === size) return;
    size = next;
    Object.assign(canvas, {
      width: size,
      height: size,
      clientWidth: size,
      clientHeight: size,
    });
    target.Configure({ width: size, height: size });
    const present = tri.GetPresentParameters();
    tri.SetPresentParameters(0, {
      ...present,
      mode: { ...present.mode, width: size, height: size },
    });
    tri.width = size;
    tri.height = size;
    tri.InitD3DDevice();
  }

  // Loads only advance on Blue ticks.
  const pump = setInterval(() => blue.os.PumpOS(), PUMP_INTERVAL_MS);
  const loaded = await blue.resMan.LoadObject(SCENE).catch(() => null);
  clearInterval(pump);
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
    Run: () =>
      driver.Execute(null, null, FIXED_TIME, FIXED_TIME, null, renderContext),
  });
  placeSun(scene, THUMBNAIL_POSE.yaw, THUMBNAIL_POSE.pitch);

  {
    const resMan: any = blue.resMan;
    for (const method of ["_GetResource", "_LoadObject"]) {
      const original = resMan[method].bind(resMan);
      resMan[method] = (path: unknown, ...rest: unknown[]) => {
        const name = String(path).toLowerCase();
        if (name.startsWith("res:/") && !used.has(name)) {
          used.add(name);
          requests += 1;
        }
        return original(path, ...rest);
      };
    }
  }

  /**
   * Draws frames until one shows everything: a frame that started with
   * nothing loading and asked for nothing new while it was drawn (a ship
   * requests its files as it is drawn, and a load can finish between two
   * frames). False on a timeout.
   */
  async function settle() {
    for (let frame = 0; frame < MAX_FRAMES; frame++) {
      const idle = !blue.resMan.IsLoading();
      const before = requests;
      blue.os.PumpOS();
      // One frame at a time is the point: each waits on the last.
      // oxlint-disable-next-line no-await-in-loop
      await device.queue.onSubmittedWorkDone();
      if (idle && requests === before && !blue.resMan.IsLoading()) return true;
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
    if (!(await settle()))
      throw new Error("still loading after the frame limit");
    if (gpuErrors.length)
      throw new Error(`GPU error: ${gpuErrors[0].slice(0, 200)}`);
  }

  /** Copies the last frame back as straight-alpha RGBA. */
  async function readFrame() {
    // Rows of a texture copy are padded to 256 bytes.
    const bytesPerRow = Math.ceil((size * 4) / 256) * 256;
    const buffer = device.createBuffer({
      size: bytesPerRow * size,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const encoder = device.createCommandEncoder();
    encoder.copyTextureToBuffer(
      { texture: frameTexture! },
      { buffer, bytesPerRow },
      [size, size],
    );
    device.queue.submit([encoder.finish()]);
    await buffer.mapAsync(GPUMapMode.READ);
    const padded = new Uint8Array(buffer.getMappedRange());
    const pixels = new Uint8ClampedArray(size * size * 4);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const from = y * bytesPerRow + x * 4;
        const to = (y * size + x) * 4;
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
    let [left, top, right, bottom] = [size, size, -1, -1];
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        if (pixels[(y * size + x) * 4 + 3] < SOLID) continue;
        left = Math.min(left, x);
        right = Math.max(right, x);
        top = Math.min(top, y);
        bottom = Math.max(bottom, y);
      }
    }
    return right < 0
      ? null
      : { left, top, right: right + 1, bottom: bottom + 1 };
  }

  async function png(pixels: Uint8ClampedArray) {
    const blob = await encodePng(pixels, size, 0, 0, size, size);
    return new Uint8Array(await blob.arrayBuffer());
  }

  async function render(
    dna: string,
    themes: readonly Theme[],
    wanted = options.size,
  ) {
    resize(wanted);
    missing = [];
    used.clear();
    gpuErrors = [];
    random.crtSrand(RANDOM_SEED);
    random.triSrand(RANDOM_SEED);
    const values = await sof.BuildValuesFromDNAAsync(dna);
    const ship = buildShip(values);
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
      for (let update = 0; update < WARM_UP_UPDATES; update++) {
        scene.Update(FIXED_TIME, FIXED_TIME);
      }
      // A frame with the effects at their level (nothing is left to load).
      await ready();
      const box = hullBox(await readFrame());
      if (!box) throw new Error("nothing was drawn");

      // Narrow the view onto that box, made square and padded: the same
      // camera, angle and perspective, only cropped to the hull.
      const half = near * Math.tan(FIELD_OF_VIEW / 2);
      const toPlane = (pixel: number) => (pixel / size) * 2 * half - half;
      const centerX = toPlane((box.left + box.right) / 2);
      const centerY = -toPlane((box.top + box.bottom) / 2);
      const extent =
        (Math.max(box.right - box.left, box.bottom - box.top) / size) *
        half *
        PADDING;
      projection.PerspectiveOffCenter(
        centerX - extent,
        centerX + extent,
        centerY - extent,
        centerY + extent,
        near,
        far,
      );

      const pngs: Shot["pngs"] = {};
      // Themes with the same look (by default they all have the studio's)
      // share one picture instead of each drawing it again.
      const drawn = new Map<Look, Uint8Array>();
      for (const theme of themes) {
        const look = looks[theme];
        let picture = drawn.get(look);
        if (!picture) {
          applyLighting(scene, lighting, look);
          // Everything is loaded by now; this only lets a few frames through.
          // oxlint-disable-next-line no-await-in-loop
          await ready();
          // oxlint-disable-next-line no-await-in-loop
          picture = await png(await readFrame());
          drawn.set(look, picture);
        }
        pngs[theme] = picture;
      }
      const failed = new Set(missing.map((path) => path.toLowerCase()));
      return {
        pngs,
        used: [...used].filter((path) => !failed.has(path)).sort(),
        missing: [...missing],
      };
    } finally {
      scene.objects.splice(scene.objects.indexOf(ship), 1);
    }
  }

  return {
    render,
    registerPattern(name: string, record: object) {
      sof.dataMgr.UpdatePattern(name, record);
    },
  };
}
