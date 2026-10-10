// Draws one ship with the runtime's own WebGPU backend and EVE's shaders,
// translated in the browser. The runtime is pre-alpha; every workaround for
// a missing piece is commented where it happens.
import { blue } from '@carbonenginejs/runtime/blue';
import {
  RegisterGeometryResources,
  RegisterTextureResources,
  RegisterShaderResources,
  RegisterTextureArray,
  RegisterTexturePack,
  RegisterSolidColorTexture,
} from '@carbonenginejs/runtime/resource';
import { RegisterObjectResources } from '@carbonenginejs/runtime/resource/object';
import { CjsWebgpuFormat } from '@carbonenginejs/runtime/resource/formats/webgpu';
import { CjsPngFormat } from '@carbonenginejs/runtime/resource/formats/png';
import { SetEffectPathDefaults } from '@carbonenginejs/runtime/utils';
import { CjsSchema } from '@carbonenginejs/runtime/schema';
import '@carbonenginejs/runtime/trinity';
// Ships carry sound emitters; the audio classes must exist to hydrate them (audio stays headless).
import '@carbonenginejs/runtime/audio/trinity';
import {
  TriDevice,
  Tr2RenderContext_GetMainThreadRenderContext,
  TriView,
  TriProjection,
  CjsBatchManager,
} from '@carbonenginejs/runtime/trinity/core';
import { EveSpaceScene } from '@carbonenginejs/runtime/trinity/eve';
import { EveSpaceSceneRenderDriver } from '@carbonenginejs/runtime/trinity';
import { CjsWebgpuDevice } from '@carbonenginejs/runtime/trinityal/webgpu';
import { CjsWebgpuRenderContextAL, CjsWebgpuRenderTarget } from 'cjs-webgpu-internal';

import { readBytes, type FileIndexLibrary } from './resources';
import { HERO_POSE, REST_PITCH, TRANSITION_MS, easeInOut, type CameraPose } from './poses';
import { Hologram, gpuUsage } from './hologram';
import { encodePng } from './png';

/* eslint-disable @typescript-eslint/no-explicit-any */

/** A Caldari region backdrop: sun direction, ambient and reflection maps. */
const DEFAULT_SCENE = 'res:/dx9/scene/universe/c01_cube.red';

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

/** How far from a ship the camera starts, in bounding-sphere radii. */
const FRAMING_RADII = 3.2;
/** Radians per dragged pixel. */
const ORBIT_SPEED = 0.008;
/** Radians per second the camera drifts until the viewer touches it. */
const IDLE_SPIN = 0.1;
/** A new ship flies in from this many times its resting distance. */
const FLY_IN_FACTOR = 2.4;
/** How quickly the camera eases toward its resting distance, per second. */
const DISTANCE_EASING = 2.2;
/** How quickly the camera eases back to its resting pitch, per second. */
const PITCH_EASING = 1.2;
/** The resting angle: slightly above the ship, three-quarter view. */
const REST_YAW = 0.6;
interface CameraState {
  yaw: number;
  pitch: number;
  zoom: number;
  blueprint: number;
}

/** How quickly the hologram fades in and out, per second. */
const HOLOGRAM_EASING = 3;
/** How quickly the missing-resource glitch comes and goes, per second. */
const GLITCH_EASING = 6;

const MAX_PITCH = 1.5;
const FIELD_OF_VIEW = Math.PI / 4;
/** Livery carousel: the two ships never sit closer than this many combined radii. */
const SLIDE_SPACING = 1.2;
/**
 * How far past the edge of the frame a ship off to the side sits, in its
 * radii: the incoming one while it loads, the outgoing one once the glide
 * ends. Bounding spheres are rough for long hulls (a bow turned toward the
 * edge reaches well past them), so this leaves generous room.
 */
const EDGE_MARGIN = 1.4;
/** How long the camera takes to glide onto the incoming ship, in ms. */
const SLIDE_DURATION = 1300;
/** Frames the resource manager must stay idle before the incoming ship counts as loaded. */
const QUIET_FRAMES = 8;
/** Slide anyway after this long, in ms, rather than wait on a resource that never lands. */
const READY_TIMEOUT = 12000;
/**
 * Blueprint views use a narrow field of view from far away: close enough to
 * orthographic that the three views share one scale, as a drawing should.
 */
const BLUEPRINT_FOV = (6 * Math.PI) / 180;
/** Room around the ship in a blueprint view, in bounding-sphere radii. */
const BLUEPRINT_MARGIN = 1.08;
/** Pixels of padding kept around the ship when a view is cropped. */
const BLUEPRINT_PADDING = 6;
/**
 * The drawn views. The side elevation is the live ship itself (see
 * setBlueprint), seen from the same yaw as the plan, so length runs left to
 * right in both.
 */
const BLUEPRINT_VIEWS = [
  // Also drawn statically, so the sheet keeps its ship once the live one leaves.
  { name: 'side', yaw: Math.PI / 2, pitch: 0 },
  { name: 'top', yaw: Math.PI / 2, pitch: Math.PI / 2 },
  { name: 'front', yaw: 0, pitch: 0 },
] as const;

export interface BlueprintView {
  name: (typeof BLUEPRINT_VIEWS)[number]['name'];
  url: string;
  /** Cropped size in canvas pixels; every view shares one scale. */
  width: number;
  height: number;
  /** Where the ship's centre falls in the cropped image, in its pixels. */
  centerX: number;
  centerY: number;
}

export interface Blueprint {
  views: BlueprintView[];
  /** The drawings' scale. */
  metersPerPixel: number;
  /** Bounding-box size in metres, from the plan (length, beam) and front (height). */
  length: number;
  beam: number;
  height: number;
  /**
   * How far the hull reaches from its centre, in metres, along its length and
   * up or down: the box the live side elevation has to fit, centred on the ship.
   */
  halfLength: number;
  halfHeight: number;
}

/** Canvas pixels per CSS pixel, capped to keep 4K screens affordable. */
const MAX_PIXEL_RATIO = 2;

/** A fixed world position in the shape space objects read their translation curve in. */
function placement(position: number[]) {
  return {
    position,
    GetValueAt(_time: number, out: number[]) {
      out[0] = position[0];
      out[1] = position[1];
      out[2] = position[2];
      return out;
    },
  };
}

/** Crops a captured view to the hull's visible pixels; null when nothing was drawn. */
async function crop(image: ImageData) {
  const { width, height, data } = image;
  let left = width;
  let right = -1;
  let top = height;
  let bottom = -1;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (data[(y * width + x) * 4 + 3] > 8) {
        if (x < left) left = x;
        if (x > right) right = x;
        if (y < top) top = y;
        if (y > bottom) bottom = y;
      }
    }
  }
  if (right < 0) return null;
  left = Math.max(0, left - BLUEPRINT_PADDING);
  top = Math.max(0, top - BLUEPRINT_PADDING);
  const cropWidth = Math.min(width, right + BLUEPRINT_PADDING + 1) - left;
  const cropHeight = Math.min(height, bottom + BLUEPRINT_PADDING + 1) - top;
  const png = await encodePng(data, width, left, top, cropWidth, cropHeight);
  return {
    url: URL.createObjectURL(png),
    width: cropWidth,
    height: cropHeight,
    // The camera looks at the ship's centre, which is the frame's centre.
    centerX: width / 2 - left,
    centerY: height / 2 - top,
  };
}

/**
 * The livery section's view: its camera pose, whether it is on screen, and
 * where the ship sits in it. The canvas spans the whole viewport; `shift`
 * moves the ship right by that share of its width (an off-centre lens, so
 * nothing is cut off at a frame edge) and `scale` sizes it.
 */
export interface LiveryView {
  pose: CameraPose;
  visible: boolean;
  shift: number;
  scale: number;
}

/** Ease-in-out with a soft landing, for the carousel glide. */
const glide = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

const nextFrame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));

/**
 * The resource manager reads every res:/ path through the file index. A read
 * that fails (not in the index, or the CDN refused it) is reported to
 * `onMissing` before the runtime gets the error.
 */
function createSource(library: FileIndexLibrary, onMissing: (path: string, error: Error) => void) {
  return {
    // Shaders keep their webgpu path; the bytes behind it are the shipped dx11 effect.
    Read: (path: string) =>
      readBytes(library, path.replace('/effect.webgpu/', '/effect.dx11/')).catch((error: Error) => {
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
  try {
    const scene = await blue.resMan.LoadObject(path);
    if (scene instanceof EveSpaceScene) return scene;
    console.warn('Not an EveSpaceScene:', path);
  } catch (error) {
    console.warn(`Could not load ${path}`, error);
  }
  return new EveSpaceScene();
}

export class ShipRenderer {
  /** Owned by the renderer, so it can move between pages without a new device. */
  readonly canvas = document.createElement('canvas');
  ship: any = null;

  private yaw = REST_YAW;
  private pitch = REST_PITCH;
  /** Where the camera sits now, and where it is easing to. */
  private distance = 150;
  private restDistance = 150;
  private pitchTarget: number | null = null;
  private yawTarget: number | null = null;
  /** The pose the page asked for last; a new ship flies in onto it. */
  private pose: CameraPose = HERO_POSE;
  /** Whether the idle spin resumes once the camera reaches its yaw target. */
  private spinAfterTarget = false;
  /** A timed camera transition in progress: it ends exactly on `to`. */
  private tween: { from: CameraState; to: CameraState; start: number; spinAfter: boolean } | null =
    null;
  /** The pose's distance factor, and where it is easing to. */
  private zoom = 1;
  private zoomTarget = 1;
  /** 0 is the normal render, 1 the fitting window hologram. */
  private hologramAmount = 0;
  private hologramTarget = 0;
  private hologram: Hologram | null = null;
  /** 0 is a clean picture, 1 the full missing-resource glitch. */
  private glitchAmount = 0;
  private glitchTarget = 0;
  /** Called for every resource read that fails, with its res:/ path. */
  onMissing: ((path: string, error: Error) => void) | null = null;
  /**
   * The livery section's own view: a second canvas showing the ship from its
   * own pose with the normal render, drawn by rendering the scene a second
   * time each frame and copying the result over on the GPU. It lets the main
   * camera stay on the blueprint while the liveries are on screen.
   */
  readonly liveryCanvas = document.createElement('canvas');
  private liveryContext: any = null;
  private liveryView: LiveryView | null = null;
  /** The camera's field of view; blueprint captures narrow it for a moment. */
  private fov = FIELD_OF_VIEW;
  /**
   * The blueprint framing: 0 is the normal camera, 1 the near-orthographic
   * side elevation showing `blueprintHalfHeight` metres above and below the
   * ship's centre. Eased, so the camera morphs between them.
   */
  private blueprintAmount = 0;
  /** How far right of centre the camera's lens is shifted, as a share of the canvas width. */
  private lensShift = 0;
  /** The livery view's own idle spin, for a pose without a yaw. */
  private liveryYaw = 0;
  /** Where the viewer dragged the livery view to; null follows its pose. */
  private liveryAngles: { yaw: number; pitch: number } | null = null;
  private liveryDrag: { x: number; y: number } | null = null;
  private blueprintTarget = 0;
  private blueprintHalfHeight = 0;
  private target = [0, 0, 0];
  private spinning = true;
  private lastFrame = 0;
  private drag: { x: number; y: number } | null = null;
  /** The resting radius of the ship on stage, for spacing the carousel. */
  private shipRadius = 50;
  /** Bumped by setShip: a different ship cancels every livery slide. */
  private slideToken = 0;
  /**
   * The livery waiting to slide in next. Only one waits: a newer request
   * replaces it, so skipped liveries never play.
   */
  private nextSlide: {
    values: any;
    direction: 1 | -1;
    resolve: (shown: boolean) => void;
  } | null = null;
  /** Whether the queue is working through slides. */
  private sliding = false;
  /** The running glide, if any: the camera target moves from one ship to the next. */
  private slide: {
    from: number[];
    to: number[];
    fromDistance: number;
    toDistance: number;
    /** The outgoing ship's position, moved outward along `push` as the glide runs. */
    outgoing: number[];
    push: number[];
    start: number;
    resolve: () => void;
  } | null = null;
  private slideDone: Promise<void> = Promise.resolve();
  private reported = new Set<string>();

  private device: any;
  private al: any;
  private scene: any;
  private view: any;
  private projection: any;
  private driver: any;

  static async create(library: FileIndexLibrary): Promise<ShipRenderer> {
    if (!navigator.gpu) throw new Error('This browser has no WebGPU.');
    const renderer = new ShipRenderer();
    await renderer.init(library);
    return renderer;
  }

  private async init(library: FileIndexLibrary) {
    blue.resMan.Register({
      source: createSource(library, (path, error) => this.onMissing?.(path, error)),
      // Formats are opt-in; without PNG, png textures (the film grain noise)
      // fail as an unrecognized image type.
      formats: [CjsPngFormat],
      useWorkerLoading: false,
    });
    RegisterGeometryResources(blue.resMan);
    RegisterTextureResources(blue.resMan);
    // Composite textures some hulls reference as dynamic:/ paths (the Amarr
    // titans' plating detail arrays, for one); without these the whole frame fails.
    RegisterTextureArray(blue.resMan);
    RegisterTexturePack(blue.resMan);
    RegisterSolidColorTexture(blue.resMan);
    RegisterObjectResources(blue.resMan);
    RegisterShaderResources(blue.resMan, { translator: CjsWebgpuFormat, permutations: 'onDemand' });
    SetEffectPathDefaults({ platformName: 'webgpu', shaderModel: 'high' });

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

    const backBuffer = new CjsWebgpuRenderTarget(webgpu, {
      canvas: this.canvas,
      depthFormat: 'depth32float',
      alphaMode: 'premultiplied',
      // The hologram pass copies the finished frame aside.
      extraUsage: gpuUsage.GPUTextureUsage.COPY_SRC,
    });
    clearCanvasEachScene(backBuffer);
    // The format is chosen when the canvas is first configured.
    backBuffer.Configure({ width: 1, height: 1 });
    this.hologram = new Hologram(webgpu.GetDevice(), this.canvas, backBuffer.GetFormat());
    this.liveryContext = this.liveryCanvas.getContext('webgpu');
    this.liveryContext?.configure({
      device: webgpu.GetDevice(),
      format: backBuffer.GetFormat(),
      alphaMode: 'premultiplied',
      usage: gpuUsage.GPUTextureUsage.COPY_DST | gpuUsage.GPUTextureUsage.RENDER_ATTACHMENT,
    });
    this.al = new CjsWebgpuRenderContextAL({ webgpu, renderTarget: backBuffer });
    Tr2RenderContext_GetMainThreadRenderContext().SetRenderContextAL(this.al);

    this.device = new TriDevice();
    if (!this.device.CreateSimpleDevice(this.canvas, 1, 1)) {
      throw new Error('Could not create the Trinity device.');
    }

    this.scene = await loadScene(DEFAULT_SCENE);
    this.view = new TriView();
    this.projection = new TriProjection();

    this.driver = new EveSpaceSceneRenderDriver();
    this.driver.scene = this.scene;
    this.driver.view = this.view;
    this.driver.projection = this.projection;
    this.driver.clearColor = CLEAR_COLOR;
    // Without a batch manager the driver collects nothing. With no producers
    // registered, every renderable falls back to its own GetBatches.
    this.driver.SetBatchManager(new CjsBatchManager().Initialize());

    // The WebGPU backend cannot hand its canvas to a Tr2RenderTarget yet, so
    // the driver runs without a destination and draws into the bound canvas
    // pass. That also skips post-processing, which the runtime does not drive yet.
    const renderContext = Tr2RenderContext_GetMainThreadRenderContext();
    this.device.SetRenderJobs({
      RunUpdate: () => {},
      Run: (realTime: number, simTime: number) =>
        this.driver.Execute(null, null, realTime, simTime, null, renderContext),
    });

    this.bindInput();
    new ResizeObserver(() => this.resize()).observe(this.canvas);
    requestAnimationFrame((time) => this.frame(time));
  }

  /** Swaps the shown ship for one built from SOF model values. */
  setShip(values: any) {
    this.slideToken++;
    this.nextSlide?.resolve(false);
    this.nextSlide = null;
    // A different ship cancels any livery glide in flight.
    if (this.slide) {
      const { resolve } = this.slide;
      this.slide = null;
      resolve();
    }
    this.removeObject(this.ship);
    this.ship = CjsSchema.from('EveShip2', values);
    this.ship.translationCurve = placement([0, 0, 0]);
    this.scene.objects.push(this.ship);
    this.target = [...(values.boundingSphereCenter ?? [0, 0, 0])];
    this.shipRadius = values.boundingSphereRadius ?? 50;
    this.restDistance = Math.max(10, this.shipRadius * FRAMING_RADII);
    // Fly in from afar and a little higher, onto the pose the page asked
    // for (not a fixed one), unless the viewer is mid-drag.
    if (!this.drag) {
      this.distance = this.restDistance * FLY_IN_FACTOR;
      this.pitch = this.pose.pitch + 0.35;
      this.pitchTarget = this.pose.pitch;
    }
  }

  /**
   * The livery carousel. The new ship is built beside the current one, out of
   * frame on the side it comes from; once everything it needs has loaded, the
   * camera glides over to it, the old ship leaves on the other side and is
   * dropped. Resolves false when a newer request overtook this one.
   */
  async slideToShip(values: any, direction: 1 | -1): Promise<boolean> {
    // A livery that has not started gliding yet gives way to this one.
    this.nextSlide?.resolve(false);
    const shown = new Promise<boolean>((resolve) => {
      this.nextSlide = { values, direction, resolve };
    });
    void this.runSlides();
    return shown;
  }

  /**
   * Plays queued slides one after another. A glide that has started always
   * finishes, so rapid clicks never leave the carousel halfway; whatever was
   * clicked meanwhile plays next (only the latest of it).
   */
  private async runSlides() {
    if (this.sliding) return;
    this.sliding = true;
    try {
      while (this.nextSlide) {
        const { values, direction, resolve } = this.nextSlide;
        this.nextSlide = null;
        // One slide at a time is the point of the queue.
        // oxlint-disable-next-line no-await-in-loop
        resolve(await this.slideOnce(values, direction));
      }
    } finally {
      this.sliding = false;
    }
  }

  private async slideOnce(values: any, direction: 1 | -1): Promise<boolean> {
    const token = this.slideToken;
    if (!this.ship) {
      this.setShip(values);
      return true;
    }

    const radius = values.boundingSphereRadius ?? 50;
    const center: number[] = values.boundingSphereCenter ?? [0, 0, 0];
    const fromDistance = this.distance;
    const toDistance = Math.max(10, radius * FRAMING_RADII);

    // The camera the viewer watches the carousel through: the livery view
    // while it shows, else the main one.
    const livery = this.liveryView?.visible ? this.liveryView : null;
    const yaw = livery ? this.liveryCamera(livery.pose).yaw : this.yaw;
    const zoom = livery ? (livery.pose.zoom ?? 1) / livery.scale : 1;
    const shift = livery?.shift ?? 0;

    // Spaced by what the camera can see, not by ship size alone: a big hull
    // is framed from further back, where the view is wider. With the ship
    // off centre, the side it sits nearer to has less room than the other.
    const aspect = this.canvas.width / Math.max(1, this.canvas.height);
    const reach = (distance: number, side: 1 | -1) =>
      distance * zoom * Math.tan(FIELD_OF_VIEW / 2) * aspect * (1 - 2 * shift * side);
    const spacing = Math.max(
      (this.shipRadius + radius) * SLIDE_SPACING,
      reach(fromDistance, direction) + radius * EDGE_MARGIN,
    );
    // Whatever the gap leaves of the outgoing ship inside the final frame, it
    // travels outward during the glide, so it has left the picture before it
    // is removed.
    const push = Math.max(
      0,
      reach(toDistance, direction === 1 ? -1 : 1) + this.shipRadius * EDGE_MARGIN - spacing,
    );

    const right = [Math.cos(yaw), 0, -Math.sin(yaw)];
    // Where the incoming ship's visual centre goes: beside the point the
    // camera looks at now. Its bounding-sphere centre sits off its origin
    // (usually along its length), so the origin is placed to cancel that out;
    // otherwise the spin would push one side's ship further into view.
    const destination = this.target.map((value, axis) => value + right[axis] * spacing * direction);
    const offset = destination.map((value, axis) => value - center[axis]);

    const incoming = CjsSchema.from('EveShip2', values);
    incoming.translationCurve = placement(offset);
    this.scene.objects.push(incoming);

    if (!(await this.waitForResources(token))) {
      this.removeObject(incoming);
      return false;
    }

    // Glide: the target moves to the incoming ship and the frame re-fits it,
    // while the outgoing ship drifts the other way.
    const outgoing = this.ship.translationCurve?.position ?? [0, 0, 0];
    this.slideDone = new Promise<void>((resolve) => {
      this.slide = {
        from: [...this.target],
        to: destination,
        fromDistance,
        toDistance,
        outgoing,
        push: right.map((axis) => -axis * push * direction),
        start: performance.now(),
        resolve,
      };
    });
    await this.slideDone;
    if (token !== this.slideToken) {
      // Cut short by a new ship; that ship replaced everything already.
      this.removeObject(incoming);
      return false;
    }

    // Swap roles and recenter on the origin, moving ship and camera target
    // together so nothing on screen jumps.
    this.removeObject(this.ship);
    this.ship = incoming;
    this.shipRadius = radius;
    incoming.translationCurve = placement([0, 0, 0]);
    this.target = [...center];
    return true;
  }

  private removeObject(object: any) {
    if (!object) return;
    const at = this.scene.objects.indexOf(object);
    if (at !== -1) this.scene.objects.splice(at, 1);
  }

  /** Waits until the resource manager has stayed idle for a few frames. */
  private async waitForResources(token: number): Promise<boolean> {
    const deadline = performance.now() + READY_TIMEOUT;
    let quiet = 0;
    while (performance.now() < deadline) {
      // Polling once per frame is the point here, not a missed parallelism.
      // oxlint-disable-next-line no-await-in-loop
      await nextFrame();
      // A different ship, or a newer livery while this one still loads.
      if (token !== this.slideToken || this.nextSlide) return false;
      quiet = blue.resMan.IsLoading() ? 0 : quiet + 1;
      if (quiet >= QUIET_FRAMES) return true;
    }
    return token === this.slideToken && !this.nextSlide;
  }

  /** Back to the starting angle and idle spin. */
  resetView() {
    this.setPose(HERO_POSE);
    this.distance = Math.max(this.distance, this.restDistance);
  }

  /**
   * Draws the ship's blueprint: side, top and front views in the hologram
   * style, from a near-orthographic camera, each cropped to the hull. Renders
   * them all within one task and then the live view again, so nothing flashes.
   */
  async captureBlueprint(): Promise<Blueprint | null> {
    if (!this.ship || !this.hologram) return null;
    await this.whenIdle();
    if (!this.ship) return null;
    const hologram = this.hologram;

    const saved = {
      yaw: this.yaw,
      pitch: this.pitch,
      distance: this.distance,
      zoom: this.zoom,
      blueprintAmount: this.blueprintAmount,
    };
    const distance = (this.shipRadius / Math.tan(BLUEPRINT_FOV / 2)) * BLUEPRINT_MARGIN;
    const reads: Promise<ImageData>[] = [];
    try {
      this.fov = BLUEPRINT_FOV;
      this.distance = distance;
      this.zoom = 1;
      this.blueprintAmount = 0;
      for (const view of BLUEPRINT_VIEWS) {
        this.yaw = view.yaw;
        this.pitch = view.pitch;
        this.updateCamera();
        this.pump();
        hologram.apply(1, 0);
        reads.push(hologram.readback());
      }
    } finally {
      this.fov = FIELD_OF_VIEW;
      Object.assign(this, saved);
      this.updateCamera();
      this.pump();
      if (this.hologramAmount > 0) hologram.apply(this.hologramAmount, 0);
    }

    const images = await Promise.all(reads);
    const views: BlueprintView[] = [];
    const cropped = await Promise.all(images.map((image) => crop(image)));
    BLUEPRINT_VIEWS.forEach((view, index) => {
      const found = cropped[index];
      if (found) views.push({ name: view.name, ...found });
    });
    const metersPerPixel = (2 * distance * Math.tan(BLUEPRINT_FOV / 2)) / this.canvas.height;
    const top = views.find((view) => view.name === 'top');
    const front = views.find((view) => view.name === 'front');
    return {
      views,
      metersPerPixel,
      length: (top?.width ?? 0) * metersPerPixel,
      beam: (top?.height ?? 0) * metersPerPixel,
      height: (front?.height ?? 0) * metersPerPixel,
      halfLength: top ? Math.max(top.centerX, top.width - top.centerX) * metersPerPixel : 0,
      halfHeight: front
        ? Math.max(front.centerY, front.height - front.centerY) * metersPerPixel
        : 0,
    };
  }

  /**
   * Morphs the camera into the blueprint's side elevation, framed so the
   * canvas shows `halfHeight` metres above and below the ship's centre, or
   * back to the normal camera with null.
   */
  setBlueprint(halfHeight: number | null) {
    const target = halfHeight === null ? 0 : 1;
    if (halfHeight !== null) this.blueprintHalfHeight = halfHeight;
    this.blueprintTarget = target;
    if (this.tween) {
      this.tween.to.blueprint = target;
    } else if (this.blueprintAmount !== target) {
      this.startTween({ ...this.cameraState(), blueprint: target }, this.spinning);
    }
  }

  /** Resolves once the resource manager has stayed idle for a few frames. */
  private async whenIdle() {
    const deadline = performance.now() + READY_TIMEOUT;
    let quiet = 0;
    while (performance.now() < deadline && quiet < QUIET_FRAMES) {
      // Polling once per frame is the point here.
      // oxlint-disable-next-line no-await-in-loop
      await nextFrame();
      quiet = blue.resMan.IsLoading() ? 0 : quiet + 1;
    }
  }

  /**
   * Shows the ship in the blueprint's canvas, framed so the canvas spans the
   * hull's length (`halfLength` either side of its centre) and `halfHeight`
   * above and below; null stops it. Only rendered while `visible`.
   */
  /** Shows the livery view from `pose` while `visible`, or stops it with null. */
  setLiveryView(view: LiveryView | null) {
    this.liveryView = view;
  }

  /** The livery view's camera angles: dragged, or its pose (spinning without a yaw). */
  private liveryCamera(pose: CameraPose) {
    return this.liveryAngles ?? { yaw: pose.yaw ?? this.liveryYaw, pitch: pose.pitch };
  }

  /** Renders the livery view into the main canvas and copies it to its own canvas. */
  private renderLiveryView(view: LiveryView, time: number) {
    const { pose } = view;
    if (!this.hologram || !this.liveryContext) return;
    const { width, height } = this.canvas;
    if (this.liveryCanvas.width !== width || this.liveryCanvas.height !== height) {
      this.liveryCanvas.width = width;
      this.liveryCanvas.height = height;
    }
    const saved = {
      yaw: this.yaw,
      pitch: this.pitch,
      zoom: this.zoom,
      blueprintAmount: this.blueprintAmount,
      lensShift: this.lensShift,
    };
    try {
      ({ yaw: this.yaw, pitch: this.pitch } = this.liveryCamera(pose));
      // A smaller ship is the same ship seen from further back.
      this.zoom = (pose.zoom ?? 1) / view.scale;
      this.lensShift = view.shift;
      this.blueprintAmount = 0;
      this.updateCamera();
      blue.os.PumpOS();
      if (this.glitchAmount > 0) this.hologram.apply(0, time / 1000, this.glitchAmount);
      this.hologram.copyTo(this.liveryContext, { x: 0, y: 0, width, height });
    } finally {
      Object.assign(this, saved);
    }
  }

  /** Jumps every eased camera value to where it is heading. */
  settle() {
    this.finishTween();
    this.blueprintAmount = this.blueprintTarget;
    this.hologramAmount = this.hologramTarget;
    if (this.yawTarget !== null) this.yaw = this.yawTarget;
    if (this.pitchTarget !== null) this.pitch = this.pitchTarget;
    this.zoom = this.zoomTarget;
    this.distance = this.restDistance;
  }

  /** Glitches the picture (1) for a ship with missing resources, or clears it (0). */
  setGlitch(amount: number) {
    this.glitchTarget = Math.min(1, Math.max(0, amount));
  }

  /** Fades toward the fitting window hologram (1) or the normal render (0). */
  setHologram(amount: number) {
    this.hologramTarget = Math.min(1, Math.max(0, amount));
  }

  private cameraState(): CameraState {
    return { yaw: this.yaw, pitch: this.pitch, zoom: this.zoom, blueprint: this.blueprintAmount };
  }

  /**
   * Moves the camera onto a pose over TRANSITION_MS, then holds it exactly
   * (a pose without a yaw resumes the idle spin). It holds until the viewer drags.
   */
  setPose(pose: CameraPose) {
    this.pose = pose;
    this.pitchTarget = null;
    this.yawTarget = null;
    let yaw = this.yaw;
    if (pose.yaw !== undefined) {
      // The shortest way round to the target angle.
      const turn = Math.PI * 2;
      yaw = this.yaw + ((((pose.yaw - this.yaw) % turn) + turn * 1.5) % turn) - turn / 2;
    }
    this.startTween(
      { yaw, pitch: pose.pitch, zoom: pose.zoom ?? 1, blueprint: this.blueprintTarget },
      pose.yaw === undefined,
    );
  }

  private startTween(to: CameraState, spinAfter: boolean) {
    this.tween = { from: this.cameraState(), to, start: performance.now(), spinAfter };
    this.zoomTarget = to.zoom;
    this.spinning = false;
  }

  /** Lands any camera transition at its end at once. */
  private finishTween() {
    const tween = this.tween;
    if (!tween) return;
    this.tween = null;
    ({
      yaw: this.yaw,
      pitch: this.pitch,
      zoom: this.zoom,
      blueprint: this.blueprintAmount,
    } = tween.to);
    this.spinning = tween.spinAfter;
  }

  private frame(time: number) {
    requestAnimationFrame((next) => this.frame(next));
    const seconds = this.lastFrame ? (time - this.lastFrame) / 1000 : 0;
    this.lastFrame = time;
    // Nothing to draw while neither canvas is on a page.
    if (!this.canvas.isConnected && !this.liveryCanvas.isConnected) return;
    if (this.spinning) this.yaw += seconds * IDLE_SPIN;
    this.liveryYaw += seconds * IDLE_SPIN;
    if (this.slide) {
      const progress = Math.min(1, (time - this.slide.start) / SLIDE_DURATION);
      const eased = glide(progress);
      const { from, to, fromDistance, toDistance, outgoing, push } = this.slide;
      this.target = from.map((value, axis) => value + (to[axis] - value) * eased);
      // The zoom follows the glide exactly, so the frame is final when it ends.
      this.distance = this.restDistance = fromDistance + (toDistance - fromDistance) * eased;
      for (let axis = 0; axis < 3; axis++) outgoing[axis] = push[axis] * eased;
      if (progress >= 1) {
        const { resolve } = this.slide;
        this.slide = null;
        resolve();
      }
    }
    this.distance +=
      (this.restDistance - this.distance) * (1 - Math.exp(-seconds * DISTANCE_EASING));
    if (this.tween) {
      const t = Math.min(1, (performance.now() - this.tween.start) / TRANSITION_MS);
      const k = easeInOut(t);
      const { from, to } = this.tween;
      this.yaw = from.yaw + (to.yaw - from.yaw) * k;
      this.pitch = from.pitch + (to.pitch - from.pitch) * k;
      this.zoom = from.zoom + (to.zoom - from.zoom) * k;
      this.blueprintAmount = from.blueprint + (to.blueprint - from.blueprint) * k;
      if (t >= 1) this.finishTween();
    }
    // The fly-in of a new ship eases its pitch on its own.
    if (this.pitchTarget !== null && !this.tween) {
      this.pitch += (this.pitchTarget - this.pitch) * (1 - Math.exp(-seconds * PITCH_EASING));
      if (Math.abs(this.pitchTarget - this.pitch) < 1e-3) this.pitchTarget = null;
    }
    this.hologramAmount +=
      (this.hologramTarget - this.hologramAmount) * (1 - Math.exp(-seconds * HOLOGRAM_EASING));
    if (Math.abs(this.hologramTarget - this.hologramAmount) < 1e-3) {
      this.hologramAmount = this.hologramTarget;
    }
    this.glitchAmount +=
      (this.glitchTarget - this.glitchAmount) * (1 - Math.exp(-seconds * GLITCH_EASING));
    if (Math.abs(this.glitchTarget - this.glitchAmount) < 1e-3) {
      this.glitchAmount = this.glitchTarget;
    }
    try {
      const view = this.liveryView;
      if (view?.visible && this.ship) this.renderLiveryView(view, time);
      this.updateCamera();
      blue.os.PumpOS();
      if (this.hologramAmount > 0 || this.glitchAmount > 0) {
        this.hologram?.apply(this.hologramAmount, time / 1000, this.glitchAmount);
      }
    } catch (error) {
      this.recoverFrame(error);
    }
  }

  /** Renders one frame through the runtime; a frame that fails is recovered from. */
  private pump() {
    try {
      blue.os.PumpOS();
      return true;
    } catch (error) {
      this.recoverFrame(error);
      return false;
    }
  }

  /**
   * A frame that threw midway leaves the WebGPU work queue open, and it then
   * refuses every later frame: close it so the next one starts clean. One
   * failed frame must not stop rendering; each distinct failure is reported once.
   */
  private recoverFrame(error: unknown) {
    // oxlint-disable-next-line no-underscore-dangle -- the queue exposes no getter for it
    if (this.al.GetWorkQueue?.()?._inFrame) {
      try {
        this.al.EndScene();
      } catch {
        // Nothing more to salvage from this frame.
      }
    }
    const message = String(error);
    if (!this.reported.has(message)) {
      this.reported.add(message);
      console.error(error);
    }
  }

  private bindInput() {
    const canvas = this.canvas;
    canvas.style.touchAction = 'none';
    canvas.addEventListener('pointerdown', (event) => {
      this.drag = { x: event.clientX, y: event.clientY };
      this.spinning = false;
      this.pitchTarget = null;
      this.yawTarget = null;
      this.tween = null;
      canvas.setPointerCapture(event.pointerId);
    });
    canvas.addEventListener('pointerup', () => (this.drag = null));
    canvas.addEventListener('pointercancel', () => (this.drag = null));
    canvas.addEventListener('pointermove', (event) => {
      if (!this.drag) return;
      this.yaw -= (event.clientX - this.drag.x) * ORBIT_SPEED;
      this.pitch = Math.min(
        MAX_PITCH,
        Math.max(-MAX_PITCH, this.pitch + (event.clientY - this.drag.y) * ORBIT_SPEED),
      );
      this.drag = { x: event.clientX, y: event.clientY };
    });
    this.bindLiveryInput();
    canvas.addEventListener(
      'wheel',
      (event) => {
        // Only zoom on purpose (ctrl/pinch or while dragging), so the page still scrolls.
        if (!event.ctrlKey && !this.drag) return;
        event.preventDefault();
        this.restDistance = Math.max(1, this.restDistance * Math.exp(event.deltaY * 0.002));
      },
      { passive: false },
    );
  }

  /**
   * Dragging the livery view orbits its own camera; the main camera, docked
   * in the blueprint, stays put. Vertical swipes still scroll the page.
   */
  private bindLiveryInput() {
    const canvas = this.liveryCanvas;
    canvas.style.touchAction = 'pan-y';
    canvas.addEventListener('pointerdown', (event) => {
      if (!this.liveryView) return;
      this.liveryAngles = this.liveryCamera(this.liveryView.pose);
      this.liveryDrag = { x: event.clientX, y: event.clientY };
      canvas.setPointerCapture(event.pointerId);
    });
    canvas.addEventListener('pointerup', () => (this.liveryDrag = null));
    canvas.addEventListener('pointercancel', () => (this.liveryDrag = null));
    canvas.addEventListener('pointermove', (event) => {
      const angles = this.liveryAngles;
      if (!this.liveryDrag || !angles) return;
      angles.yaw -= (event.clientX - this.liveryDrag.x) * ORBIT_SPEED;
      angles.pitch = Math.min(
        MAX_PITCH,
        Math.max(-MAX_PITCH, angles.pitch + (event.clientY - this.liveryDrag.y) * ORBIT_SPEED),
      );
      this.liveryDrag = { x: event.clientX, y: event.clientY };
    });
  }

  /**
   * Re-creating the device on resize trips a double free in the runtime's GPU
   * resource pool, so only the presentation surface and viewport change.
   */
  private resize() {
    const ratio = Math.min(window.devicePixelRatio || 1, MAX_PIXEL_RATIO);
    const width = Math.max(1, Math.round(this.canvas.clientWidth * ratio));
    const height = Math.max(1, Math.round(this.canvas.clientHeight * ratio));
    if (width === this.device.width && height === this.device.height) return;
    const present = this.device.GetPresentParameters();
    this.device.SetPresentParameters(0, { ...present, mode: { ...present.mode, width, height } });
    this.device.width = width;
    this.device.height = height;
    this.device.InitD3DDevice();
  }

  private updateCamera() {
    const [tx, ty, tz] = this.target;
    // Blend the framing by visible half-height rather than by distance, so the
    // ship keeps its size on screen while the field of view narrows.
    const blend = this.blueprintAmount;
    const fov = this.fov + (BLUEPRINT_FOV - this.fov) * blend;
    const normalHalfHeight = this.distance * this.zoom * Math.tan(this.fov / 2);
    const halfHeight = normalHalfHeight + (this.blueprintHalfHeight - normalHalfHeight) * blend;
    const distance = halfHeight / Math.tan(fov / 2);
    const c = Math.cos(this.pitch);
    const eye = [
      tx + distance * c * Math.sin(this.yaw),
      ty + distance * Math.sin(this.pitch),
      tz + distance * c * Math.cos(this.yaw),
    ];
    // The orbit's own up direction, so straight down is a valid view too.
    const up = [
      -Math.sin(this.pitch) * Math.sin(this.yaw),
      Math.cos(this.pitch),
      -Math.sin(this.pitch) * Math.cos(this.yaw),
    ];
    this.view.SetLookAtPosition(eye, this.target, up);
    const aspect = this.canvas.width / Math.max(1, this.canvas.height);
    const near = Math.max(0.1, distance / 100);
    const far = distance * 20;
    if (this.lensShift) {
      // The same frustum, slid sideways: what the camera looks at lands
      // `lensShift` of the width right of centre.
      const top = near * Math.tan(fov / 2);
      const half = top * aspect;
      const center = -2 * this.lensShift * half;
      this.projection.PerspectiveOffCenter(center - half, center + half, -top, top, near, far);
    } else {
      this.projection.PerspectiveFov(fov, aspect, near, far);
    }
  }
}
