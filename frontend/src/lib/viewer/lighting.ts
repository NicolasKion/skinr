// How a ship is lit, shared by the page's renderer and the thumbnail script
// (scripts/render-thumbnails.ts), so both draw it the same way.
import { EveSpaceScene } from '@carbonenginejs/runtime/trinity/eve';

/* eslint-disable @typescript-eslint/no-explicit-any */

/**
 * The sun rides with the camera, so whatever side of a ship is in view is
 * lit: it shines from this far to the left of the camera (radians of yaw)…
 */
const KEY_LIGHT_YAW = -0.7;
/** …and this far above it (radians of pitch), like a photographer's key light. */
const KEY_LIGHT_PITCH = 0.55;

/**
 * A look: the sun, ambient fill and reflections as multiples of the scene's
 * own, and the output gamma (the scene's is 1; lower lifts dark plating).
 */
export interface Look {
  sun: number;
  ambient: number;
  reflection: number;
  gamma: number;
}

/**
 * The studio look every ship is shown in, live and in thumbnails, in both
 * themes. EVE lights its scenes for space, where a hull reads as a dark
 * silhouette; like CCP's studio renders, this brings out the plating's detail.
 * Picked from a grid of variations: stronger light only washed out white hulls.
 */
export const STUDIO_LOOK: Look = { sun: 3, ambient: 5, reflection: 1.8, gamma: 0.8 };

/** A scene's own sun, ambient and reflection strength, which a look scales. */
export interface SceneLighting {
  sun: number[];
  dynamicSun: number[];
  ambient: number[];
  reflection: number;
}

/** Reads a freshly loaded scene's lighting, and detaches its sun from the system. */
export function takeLighting(scene: any): SceneLighting {
  // The scene's sun ball would put the sun back where the system has it
  // every update; the camera places it instead (placeSun).
  scene.sunBall = null;
  return {
    sun: [...scene.sunDiffuseColor],
    dynamicSun: [...scene.sunDiffuseColorWithDynamicLights],
    ambient: [...scene.ambientColor],
    reflection: scene.reflectionIntensity,
  };
}

/** Lights the scene with `look`, relative to its own lighting `base`. */
export function applyLighting(scene: any, base: SceneLighting, look: Look) {
  // Color channels only; alpha stays as the scene has it. The scene lights
  // with either sun color, depending on dynamic lighting.
  for (let channel = 0; channel < 3; channel++) {
    scene.sunDiffuseColor[channel] = base.sun[channel] * look.sun;
    scene.sunDiffuseColorWithDynamicLights[channel] = base.dynamicSun[channel] * look.sun;
    scene.ambientColor[channel] = base.ambient[channel] * look.ambient;
  }
  scene.reflectionIntensity = base.reflection * look.reflection;
  // A runtime setting rather than a scene property: every scene draws with it.
  (EveSpaceScene as any).eveSpaceSceneGammaBrightness = look.gamma;
}

/** Points the sun the way a camera at `yaw`/`pitch` looks, from just over its shoulder. */
export function placeSun(scene: any, yaw: number, pitch: number) {
  const lightYaw = yaw + KEY_LIGHT_YAW;
  const lightPitch = Math.min(Math.PI / 2 - 0.01, pitch + KEY_LIGHT_PITCH);
  const c = Math.cos(lightPitch);
  // sunDirection is the way the light travels: from the light toward the ship.
  scene.sunDirection[0] = -c * Math.sin(lightYaw);
  scene.sunDirection[1] = -Math.sin(lightPitch);
  scene.sunDirection[2] = -c * Math.cos(lightYaw);
}
