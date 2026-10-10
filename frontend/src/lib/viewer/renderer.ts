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
/**
 * The light theme's lighting. A ship lit for space reads as a dark silhouette
 * on a light page, so the sun, ambient fill and reflections are turned up (as
 * multiples of the scene's own) and the output gamma lowered (the scene's is
 * 1) to lift the dark plating. Neither does much alone; together they do.
 */
const BRIGHT_LIGHTING = { sun: 2, ambient: 2.4, reflection: 1.4, gamma: 0.7 };
/**
 * The sun rides with the camera, so whatever side of a ship is in view is
 * lit: it shines from this far to the left of the camera (radians of yaw)…
 */
const KEY_LIGHT_YAW = -0.7;
/** …and this far above it (radians of pitch), like a photographer's key light. */
const KEY_LIGHT_PITCH = 0.55;
/** How quickly the lighting moves between the two looks, per second. */
const LIGHTING_EASING = 4;

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
 * A page section's own view of the ship (the liveries, the SKINRs): its
 * camera pose, whether it is on screen, and where the ship sits in it. The
 * canvas spans the whole viewport; `shift` moves the ship right by that share
 * of its width (an off-centre lens, so nothing is cut off at a frame edge) and
 * `scale` sizes it.
 */
export interface SectionView {
  pose: CameraPose;
  visible: boolean;
  shift: number;
  scale: number;
}

/** A section view's canvas and what the viewer has done to it. */
interface Section {
  canvas: HTMLCanvasElement;
  context: any;
  view: SectionView | null;
  /** Where the viewer dragged the view to; null follows its pose. */
  angles: { yaw: number; pitch: number } | null;
  drag: { x: number; y: number } | null;
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
  /** 0 is the scene's own lighting, 1 the light theme's (BRIGHT_LIGHTING). */
  private lightingAmount = 0;
  private lightingTarget = 0;
  /** The scene's own sun, ambient and reflection strength, scaled from. */
  private baseLighting: {
    sun: number[];
    dynamicSun: number[];
    ambient: number[];
    reflection: number;
  } | null = null;
  /** Called for every resource read that fails, with its res:/ path. */
  onMissing: ((path: string, error: Error) => void) | null = null;
  /**
   * Page sections' own views, by name: further canvases showing the ship from
   * their own pose with the normal render, drawn by rendering the scene again
   * each frame and copying the result over on the GPU. They let the main
   * camera stay on the blueprint while the liveries or SKINRs are on screen.
   */
  private sections = new Map<string, Section>();
  /** The WebGPU device and canvas format, for configuring section canvases. */
  private gpuDevice: any = null;
  private canvasFormat = '';
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
  /** The section views' own idle spin, for a pose without a yaw. */
  private sectionYaw = 0;
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
  /**
   * The running glide, if any. The ships move, not the camera, and along the
   * screen's right of whichever view draws them (`placeSlide`), so each view
   * sees the new ship come in from the side it should, however it is turned.
   */
  private slide: {
    /** The incoming ship's position: beside the target while it loads, then gliding in. */
    incoming: number[];
    /** Where the incoming ship lands: its visual centre on the target. */
    landing: number[];
    /** The outgoing ship's position, gliding out the other way. */
    outgoing: number[];
    spacing: number;
    /** How much further the outgoing ship travels, so it leaves the frame. */
    push: number;
    direction: 1 | -1;
    fromDistance: number;
    toDistance: number;
    /** When the glide started; null while the incoming ship still loads. */
    start: number | null;
    eased: number;
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
    this.gpuDevice = webgpu.GetDevice();
    this.canvasFormat = backBuffer.GetFormat();
    this.al = new CjsWebgpuRenderContextAL({ webgpu, renderTarget: backBuffer });
    Tr2RenderContext_GetMainThreadRenderContext().SetRenderContextAL(this.al);

    this.device = new TriDevice();
    if (!this.device.CreateSimpleDevice(this.canvas, 1, 1)) {
      throw new Error('Could not create the Trinity device.');
    }

    this.scene = await loadScene(DEFAULT_SCENE);
    // The scene's sun ball would put the sun back where the system has it
    // every update; the camera places it instead (updateCamera).
    this.scene.sunBall = null;
    this.baseLighting = {
      sun: [...this.scene.sunDiffuseColor],
      dynamicSun: [...this.scene.sunDiffuseColorWithDynamicLights],
      ambient: [...this.scene.ambientColor],
      reflection: this.scene.reflectionIntensity,
    };
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

    // The camera whose framing spaces the ships: a section view while one
    // shows, else the main one. (Which way they glide is up to each view.)
    const watching =
      [...this.sections.values()].find((candidate) => candidate.view?.visible)?.view ?? null;
    const zoom = watching ? (watching.pose.zoom ?? 1) / watching.scale : 1;
    const shift = watching?.shift ?? 0;

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

    // The incoming ship's origin is placed so its visual centre lands on the
    // target: its bounding-sphere centre sits off its origin (usually along
    // its length), and otherwise the spin would push one side's ship further
    // into view. It waits beside the target, out of frame, while it loads.
    const landing = this.target.map((value, axis) => value - center[axis]);
    const incoming = CjsSchema.from('EveShip2', values);
    incoming.translationCurve = placement([...landing]);
    this.scene.objects.push(incoming);
    this.slideDone = new Promise<void>((resolve) => {
      this.slide = {
        incoming: incoming.translationCurve.position,
        landing,
        outgoing: this.ship.translationCurve?.position ?? [0, 0, 0],
        spacing,
        push,
        direction,
        fromDistance,
        toDistance,
        start: null,
        eased: 0,
        resolve,
      };
    });

    if (!(await this.waitForResources(token))) {
      if (token === this.slideToken) this.slide = null;
      this.removeObject(incoming);
      return false;
    }

    // Glide: the incoming ship moves onto the target and the frame re-fits
    // it, while the outgoing ship drifts out the other way.
    if (this.slide) this.slide.start = performance.now();
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

  /**
   * Places the gliding ships for a view looking from `yaw`: the incoming one
   * comes in from that view's screen side, the outgoing one leaves the other
   * way, whatever angle the view is dragged to.
   */
  private placeSlide(yaw: number) {
    const slide = this.slide;
    if (!slide) return;
    const right = [Math.cos(yaw), 0, -Math.sin(yaw)];
    const { eased, direction, spacing, push, landing, incoming, outgoing } = slide;
    for (let axis = 0; axis < 3; axis++) {
      incoming[axis] = landing[axis] + right[axis] * spacing * direction * (1 - eased);
      outgoing[axis] = -right[axis] * (spacing + push) * direction * eased;
    }
  }

  /**
   * While the incoming ship loads, draws it once in view, on its landing
   * spot, in a pass the frame's real renders then paint over. The runtime
   * only requests a ship's textures and translates its shader permutations
   * (synchronously) when it first draws it, and out of frame it is never
   * drawn: without this, a look the scene has not drawn before (a SKINR
   * pattern) would load and compile mid-glide, stalling the glide and
   * popping in. This way the wait for resources covers it, while nothing
   * on screen moves.
   */
  private warmIncoming() {
    const slide = this.slide;
    if (!slide || slide.start !== null) return;
    for (let axis = 0; axis < 3; axis++) slide.incoming[axis] = slide.landing[axis];
    this.updateCamera();
    blue.os.PumpOS();
    this.placeSlide(this.yaw);
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
   * The canvas of the section view `key`, created on first use. The page puts
   * it in its section; the renderer draws into it while the view is visible.
   */
  sectionCanvas(key: string): HTMLCanvasElement {
    return this.section(key).canvas;
  }

  /** Shows the section view `key` from its pose while `visible`, or stops it with null. */
  setSectionView(key: string, view: SectionView | null) {
    this.section(key).view = view;
  }

  private section(key: string): Section {
    let section = this.sections.get(key);
    if (!section) {
      const canvas = document.createElement('canvas');
      const context: any = canvas.getContext('webgpu');
      context?.configure({
        device: this.gpuDevice,
        format: this.canvasFormat,
        alphaMode: 'premultiplied',
        usage: gpuUsage.GPUTextureUsage.COPY_DST | gpuUsage.GPUTextureUsage.RENDER_ATTACHMENT,
      });
      section = { canvas, context, view: null, angles: null, drag: null };
      this.sections.set(key, section);
      this.bindSectionInput(section);
    }
    return section;
  }

  /** A section view's camera angles: dragged, or its pose (spinning without a yaw). */
  private sectionCamera(section: Section, pose: CameraPose) {
    return section.angles ?? { yaw: pose.yaw ?? this.sectionYaw, pitch: pose.pitch };
  }

  /** Renders a section view into the main canvas and copies it to the section's canvas. */
  private renderSectionView(section: Section, view: SectionView, time: number) {
    const { pose } = view;
    if (!this.hologram || !section.context) return;
    const { width, height } = this.canvas;
    if (section.canvas.width !== width || section.canvas.height !== height) {
      section.canvas.width = width;
      section.canvas.height = height;
    }
    const saved = {
      yaw: this.yaw,
      pitch: this.pitch,
      zoom: this.zoom,
      blueprintAmount: this.blueprintAmount,
      lensShift: this.lensShift,
    };
    try {
      ({ yaw: this.yaw, pitch: this.pitch } = this.sectionCamera(section, pose));
      // A smaller ship is the same ship seen from further back.
      this.zoom = (pose.zoom ?? 1) / view.scale;
      this.lensShift = view.shift;
      this.blueprintAmount = 0;
      this.placeSlide(this.yaw);
      this.updateCamera();
      blue.os.PumpOS();
      if (this.glitchAmount > 0) this.hologram.apply(0, time / 1000, this.glitchAmount);
      this.hologram.copyTo(section.context, { x: 0, y: 0, width, height });
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

  /** Eases toward the light theme's brighter look (1) or the scene's own (0). */
  setLighting(amount: number) {
    this.lightingTarget = Math.min(1, Math.max(0, amount));
  }

  /** Writes the lighting for the current amount into the scene. */
  private applyLighting() {
    const base = this.baseLighting;
    if (!base) return;
    const scale = (bright: number) => 1 + (bright - 1) * this.lightingAmount;
    // Color channels only; alpha stays as the scene has it. The scene lights
    // with either sun color, depending on dynamic lighting.
    for (let channel = 0; channel < 3; channel++) {
      this.scene.sunDiffuseColor[channel] = base.sun[channel] * scale(BRIGHT_LIGHTING.sun);
      this.scene.sunDiffuseColorWithDynamicLights[channel] =
        base.dynamicSun[channel] * scale(BRIGHT_LIGHTING.sun);
      this.scene.ambientColor[channel] = base.ambient[channel] * scale(BRIGHT_LIGHTING.ambient);
    }
    this.scene.reflectionIntensity = base.reflection * scale(BRIGHT_LIGHTING.reflection);
    // A runtime setting rather than a scene property: every scene draws with it.
    (EveSpaceScene as any).eveSpaceSceneGammaBrightness = scale(BRIGHT_LIGHTING.gamma);
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
    const connected = [...this.sections.values()].some((section) => section.canvas.isConnected);
    if (!this.canvas.isConnected && !connected) return;
    if (this.spinning) this.yaw += seconds * IDLE_SPIN;
    this.sectionYaw += seconds * IDLE_SPIN;
    if (this.slide?.start != null) {
      const progress = Math.min(1, (time - this.slide.start) / SLIDE_DURATION);
      const eased = glide(progress);
      const { fromDistance, toDistance } = this.slide;
      this.slide.eased = eased;
      // The zoom follows the glide exactly, so the frame is final when it ends.
      this.distance = this.restDistance = fromDistance + (toDistance - fromDistance) * eased;
      if (progress >= 1) {
        // Land exactly; the swap that follows keeps this picture.
        this.placeSlide(this.yaw);
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
    if (this.lightingAmount !== this.lightingTarget) {
      this.lightingAmount +=
        (this.lightingTarget - this.lightingAmount) * (1 - Math.exp(-seconds * LIGHTING_EASING));
      if (Math.abs(this.lightingTarget - this.lightingAmount) < 1e-3) {
        this.lightingAmount = this.lightingTarget;
      }
      this.applyLighting();
    }
    try {
      this.warmIncoming();
      for (const section of this.sections.values()) {
        if (section.view?.visible && this.ship) this.renderSectionView(section, section.view, time);
      }
      this.placeSlide(this.yaw);
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
   * Dragging a section view orbits its own camera; the main camera, docked in
   * the blueprint, stays put. Vertical swipes still scroll the page.
   */
  private bindSectionInput(section: Section) {
    const { canvas } = section;
    canvas.style.touchAction = 'pan-y';
    canvas.addEventListener('pointerdown', (event) => {
      if (!section.view) return;
      section.angles = this.sectionCamera(section, section.view.pose);
      section.drag = { x: event.clientX, y: event.clientY };
      canvas.setPointerCapture(event.pointerId);
    });
    canvas.addEventListener('pointerup', () => (section.drag = null));
    canvas.addEventListener('pointercancel', () => (section.drag = null));
    canvas.addEventListener('pointermove', (event) => {
      const { angles, drag } = section;
      if (!drag || !angles) return;
      angles.yaw -= (event.clientX - drag.x) * ORBIT_SPEED;
      angles.pitch = Math.min(
        MAX_PITCH,
        Math.max(-MAX_PITCH, angles.pitch + (event.clientY - drag.y) * ORBIT_SPEED),
      );
      section.drag = { x: event.clientX, y: event.clientY };
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

  /** Points the sun the way the camera looks, from KEY_LIGHT_YAW/PITCH off it. */
  private placeSun() {
    const yaw = this.yaw + KEY_LIGHT_YAW;
    const pitch = Math.min(Math.PI / 2 - 0.01, this.pitch + KEY_LIGHT_PITCH);
    const c = Math.cos(pitch);
    // sunDirection is the way the light travels: from the light toward the ship.
    this.scene.sunDirection[0] = -c * Math.sin(yaw);
    this.scene.sunDirection[1] = -Math.sin(pitch);
    this.scene.sunDirection[2] = -c * Math.cos(yaw);
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
    this.placeSun();
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
