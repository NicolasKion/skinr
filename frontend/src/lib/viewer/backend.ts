// Starts the runtime's own WebGPU backend with EVE's shaders, translated in
// the browser, and a scene to put a ship in. The runtime is pre-alpha; every
// workaround for a missing piece is commented where it happens.
import { blue } from '@carbonenginejs/runtime/blue';
import { CjsLibrary } from '@carbonenginejs/runtime/core';
import { CjsWebgpuFormat } from '@carbonenginejs/runtime/resource/formats/webgpu';
import imageDecodeModule from '@carbonenginejs/runtime/workers/image-decode.worker.bundle.js?url';
import compressionModule from '@carbonenginejs/runtime/workers/dds-compression.worker.bundle.js?url';
import '@carbonenginejs/runtime/trinity';
// Ships carry sound emitters; the audio classes must exist to hydrate them (audio stays headless).
import '@carbonenginejs/runtime/audio/trinity';
import {
  Tr2RenderContext_GetMainThreadRenderContext,
  TriView,
  TriProjection,
  CjsBatchManager,
} from '@carbonenginejs/runtime/trinity/core';
import { EveSpaceScene } from '@carbonenginejs/runtime/trinity/eve';
import { EveSpaceSceneRenderDriver } from '@carbonenginejs/runtime/trinity';
import { CjsWebgpuDevice } from '@carbonenginejs/runtime/trinityal/webgpu';
import { CjsWebgpuRenderContextAL, CjsWebgpuRenderTarget } from 'cjs-webgpu-internal';

import { alignBlockTextures } from './block-align';
import { Hologram, gpuUsage } from './hologram';
import { STUDIO_LOOK, applyLighting, takeLighting } from './lighting';
import type { GameFiles } from './resources';

/* eslint-disable @typescript-eslint/no-explicit-any */

/** A Caldari region backdrop: sun direction, ambient and reflection maps. */
const DEFAULT_SCENE = 'res:/dx9/scene/universe/c01_cube.red';

/** How often Blue is pumped while the scene loads, before the frame loop runs. */
const PUMP_INTERVAL_MS = 16;

/**
 * The client ships dx11 effects; the runtime translates each permutation to
 * WebGPU when it is first used.
 */
const EFFECTS = {
  source: 'dx11',
  target: 'webgpu',
  translator: CjsWebgpuFormat,
  permutations: 'onDemand',
  shaderModel: 'high',
};

/**
 * The modules BeCallbackMan imports for its jobs. The runtime's defaults
 * point at its own source files, which the build bundles away; these are the
 * single-file bundles it ships for that, copied as they are.
 */
const CALLBACK_MODULES = {
  'image.decodeBitmap': imageDecodeModule,
  'dds.compress': compressionModule,
};

/** EVE's textures are BC compressed; the rest only help when present. */
const REQUIRED_FEATURE = 'texture-compression-bc';
const OPTIONAL_FEATURES = [
  // BC compressed volume textures: Triglavian hulls, suns and cloud volumes.
  'texture-compression-bc-sliced-3d',
  'float32-filterable',
  'depth32float-stencil8',
  'rg11b10ufloat-renderable',
];

/** Fully transparent, so the page behind the canvas shows around the ship. */
const CLEAR_COLOR = [0, 0, 0, 0];

/** Canvas pixels per CSS pixel, capped to keep 4K screens affordable. */
const MAX_PIXEL_RATIO = 2;

/** Everything the renderer draws with. */
export interface Backend {
  /** The Trinity device. */
  device: any;
  /** The WebGPU render context abstraction, for recovering a failed frame. */
  al: any;
  scene: any;
  view: any;
  projection: any;
  /** The post pass over the finished frame: hologram, glitch, readback, copies. */
  hologram: Hologram;
  /** The WebGPU device and canvas format, for configuring further canvases. */
  gpu: { device: any; format: string };
}

/**
 * The resource manager reads every res:/ path through the file index. A read
 * that fails (not in the index, or the CDN refused it) is reported to
 * `onMissing` before the runtime gets the error.
 */
function createSource(files: GameFiles, onMissing: (path: string, error: Error) => void) {
  return {
    Read: (path: string) =>
      files.read(path).catch((error: Error) => {
        onMissing(path, error);
        throw error;
      }),
  };
}

/**
 * The context's Clear needs a bound texture, so on the canvas path nothing is
 * cleared and the last scene's reverse-Z depth rejects this one's fragments.
 * Clear depth to 0 (reverse-Z far) and colour to transparent on the first
 * pass of every scene.
 */
function clearCanvasEachScene(target: any) {
  const create = target.CreateRenderPassDescriptor.bind(target);
  let lastFrame: unknown = null;
  target.CreateRenderPassDescriptor = (frame: unknown, options: any = {}) => {
    // Colour too: a frame can render several views into one canvas texture.
    if (frame !== lastFrame) {
      options = {
        ...options,
        clearDepth: options.clearDepth ?? 0,
        clearColor: options.clearColor ?? { r: 0, g: 0, b: 0, a: 0 },
      };
    }
    lastFrame = frame;
    return create(frame, options);
  };
}

/** Loads one of EVE's universe scenes, or an empty scene when it cannot. */
async function loadScene(path: string) {
  // Loads only advance on Blue ticks, and the frame loop that pumps them
  // has not started yet.
  const pump = setInterval(() => blue.os.PumpOS(), PUMP_INTERVAL_MS);
  try {
    const scene = await blue.resMan.LoadObject(path);
    if (scene instanceof EveSpaceScene) return scene;
    console.warn('Not an EveSpaceScene:', path);
  } catch (error) {
    console.warn(`Could not load ${path}`, error);
  } finally {
    clearInterval(pump);
  }
  return new EveSpaceScene();
}

/** Starts the engine drawing into `canvas`, with the studio scene loaded. */
export async function createBackend(
  canvas: HTMLCanvasElement,
  files: GameFiles,
  onMissing: (path: string, error: Error) => void,
): Promise<Backend> {
  if (!navigator.gpu) throw new Error('This browser has no WebGPU.');
  // The library's defaults register every format, resource type and object
  // builder a ship needs (PNG, textures, geometry, composite textures, black).
  const runtime = await new CjsLibrary({
    resMan: { source: createSource(files, onMissing), useWorkerLoading: false },
    // Callbacks run on the main thread, pumped by PumpOS each frame.
    callbackMan: { useWorkers: false },
    callbackModules: CALLBACK_MODULES,
    effects: EFFECTS,
  }).Initialize({ sof: null, characterLibrary: null });

  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) throw new Error('No WebGPU adapter is available.');
  if (!adapter.features.has(REQUIRED_FEATURE)) {
    throw new Error("This GPU lacks BC texture compression, which EVE's textures need.");
  }
  const requiredFeatures = [REQUIRED_FEATURE, ...OPTIONAL_FEATURES].filter((feature) =>
    adapter.features.has(feature),
  );
  const webgpu = await CjsWebgpuDevice.Request({
    adapter,
    deviceDescriptor: { requiredFeatures },
  });
  alignBlockTextures(webgpu.GetDevice());

  const backBuffer = new CjsWebgpuRenderTarget(webgpu, {
    canvas,
    depthFormat: 'depth32float',
    alphaMode: 'premultiplied',
    // The hologram pass copies the finished frame aside.
    extraUsage: gpuUsage.GPUTextureUsage.COPY_SRC,
  });
  clearCanvasEachScene(backBuffer);
  // The format is chosen when the canvas is first configured.
  backBuffer.Configure({ width: 1, height: 1 });
  const format: string = backBuffer.GetFormat();
  const hologram = new Hologram(webgpu.GetDevice(), canvas, format);
  const al = new CjsWebgpuRenderContextAL({ webgpu, renderTarget: backBuffer });
  // Starts Trinity on this backend: resources prepare on trinity.device,
  // and Blue's ticks (PumpOS) drive the device's render jobs.
  const device = runtime.AttachTrinity({ renderContextAL: al });
  if (!device.CreateSimpleDevice(canvas, 1, 1)) {
    throw new Error('Could not create the Trinity device.');
  }

  const scene = await loadScene(DEFAULT_SCENE);
  applyLighting(scene, takeLighting(scene), STUDIO_LOOK);
  const view = new TriView();
  const projection = new TriProjection();

  const driver = new EveSpaceSceneRenderDriver();
  driver.scene = scene;
  driver.view = view;
  driver.projection = projection;
  driver.clearColor = CLEAR_COLOR;
  // Without a batch manager the driver collects nothing. With no producers
  // registered, every renderable falls back to its own GetBatches.
  driver.SetBatchManager(new CjsBatchManager().Initialize());

  // The WebGPU backend cannot hand its canvas to a Tr2RenderTarget yet, so
  // the driver runs without a destination and draws into the bound canvas
  // pass. That also skips post-processing, which the runtime does not drive yet.
  const renderContext = Tr2RenderContext_GetMainThreadRenderContext();
  device.SetRenderJobs({
    RunUpdate: () => {},
    Run: (realTime: number, simTime: number) =>
      driver.Execute(null, null, realTime, simTime, null, renderContext),
  });

  // Re-creating the device on resize trips a double free in the runtime's GPU
  // resource pool, so only the presentation surface and viewport change.
  const resize = () => {
    const ratio = Math.min(window.devicePixelRatio || 1, MAX_PIXEL_RATIO);
    const width = Math.max(1, Math.round(canvas.clientWidth * ratio));
    const height = Math.max(1, Math.round(canvas.clientHeight * ratio));
    if (width === device.width && height === device.height) return;
    const present = device.GetPresentParameters();
    device.SetPresentParameters(0, { ...present, mode: { ...present.mode, width, height } });
    device.width = width;
    device.height = height;
    device.InitD3DDevice();
  };
  new ResizeObserver(resize).observe(canvas);

  return {
    device,
    al,
    scene,
    view,
    projection,
    hologram,
    gpu: { device: webgpu.GetDevice(), format },
  };
}
