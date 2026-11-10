import { describe, expect, it } from 'vitest';
import { IDLE_SPIN, OrbitCamera, orbited, pinFrustum, type Rig } from './camera';
import {
  BLUEPRINT_POSE,
  FIELD_OF_VIEW,
  HERO_POSE,
  SIDE_POSE,
  THUMBNAIL_POSE,
  TRANSITION_MS,
} from './poses';

/** A rig that records what the camera wrote to it. */
function rig() {
  const written = {
    eye: [0, 0, 0],
    target: [0, 0, 0],
    fov: null as number | null,
    offCenter: null as number[] | null,
  };
  const fake: Rig = {
    view: {
      SetLookAtPosition(eye, target) {
        written.eye = eye;
        written.target = target;
      },
    },
    projection: {
      PerspectiveFov(fov) {
        written.fov = fov;
        written.offCenter = null;
      },
      PerspectiveOffCenter(left, right, bottom, top, near) {
        written.fov = null;
        written.offCenter = [left / near, right / near, bottom / near, top / near];
      },
    },
    aspect: () => 2,
    sun: () => {},
  };
  return { fake, written };
}

const distanceOf = (eye: number[], target: number[]) =>
  Math.hypot(eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]);

describe('motion', () => {
  it('spins idly until asked for anything', () => {
    const camera = new OrbitCamera();
    const yaw = camera.yaw;
    camera.update(2, 0);
    expect(camera.state.motion).toBe('spin');
    expect(camera.yaw).toBeCloseTo(yaw + 2 * IDLE_SPIN);
  });

  it('moves onto a pose with a yaw and holds it exactly', () => {
    const camera = new OrbitCamera();
    camera.setPose(SIDE_POSE, 1000);
    expect(camera.state.motion).toBe('tween');
    camera.update(0.1, 1000 + TRANSITION_MS / 2);
    expect(camera.state.motion).toBe('tween');
    camera.update(0.1, 1000 + TRANSITION_MS);
    expect(camera.state.motion).toBe('hold');
    expect(camera.pitch).toBe(SIDE_POSE.pitch);
    expect(camera.zoom).toBe(SIDE_POSE.zoom);
    // The same angle, whichever way round was shorter.
    expect(Math.cos(camera.yaw)).toBeCloseTo(Math.cos(SIDE_POSE.yaw ?? 0));
    expect(Math.sin(camera.yaw)).toBeCloseTo(Math.sin(SIDE_POSE.yaw ?? 0));
    const held = camera.yaw;
    camera.update(1, 5000);
    expect(camera.yaw).toBe(held);
  });

  it('takes the shortest way round to a pose', () => {
    const camera = new OrbitCamera();
    camera.yaw = Math.PI * 2 - 0.1;
    camera.setPose({ yaw: 0.1, pitch: 0 }, 0);
    camera.update(0, TRANSITION_MS);
    expect(camera.yaw).toBeCloseTo(Math.PI * 2 + 0.1);
  });

  it('spins again after a pose without a yaw', () => {
    const camera = new OrbitCamera();
    camera.setPose(SIDE_POSE, 0);
    camera.update(0, TRANSITION_MS);
    camera.setPose(HERO_POSE, 2000);
    camera.update(0, 2000 + TRANSITION_MS);
    expect(camera.state.motion).toBe('spin');
  });

  it('is the viewer’s once they drag, mid-move too', () => {
    const camera = new OrbitCamera();
    camera.setPose(SIDE_POSE, 0);
    camera.update(0.1, 300);
    const before = { yaw: camera.yaw, pitch: camera.pitch };
    camera.orbit(50, -20);
    expect(camera.state.motion).toBe('hold');
    expect({ yaw: camera.yaw, pitch: camera.pitch }).toEqual(orbited(before, 50, -20));
    const held = camera.yaw;
    camera.update(1, 5000);
    expect(camera.yaw).toBe(held);
  });

  it('never tilts past straight up or down', () => {
    expect(orbited({ yaw: 0, pitch: 0 }, 0, 100000).pitch).toBe(1.5);
    expect(orbited({ yaw: 0, pitch: 0 }, 0, -100000).pitch).toBe(-1.5);
  });

  it('morphs into the blueprint framing and back, keeping a running move', () => {
    const camera = new OrbitCamera();
    camera.setBlueprint(40, 0);
    expect(camera.state.motion).toBe('tween');
    camera.update(0, TRANSITION_MS);
    expect(camera.blueprint).toBe(1);
    // It was spinning before the morph, so it spins after.
    expect(camera.state.motion).toBe('spin');

    camera.setPose(BLUEPRINT_POSE, 2000);
    camera.setBlueprint(null, 2100);
    camera.update(0, 2000 + TRANSITION_MS);
    expect(camera.blueprint).toBe(0);
    expect(camera.state.motion).toBe('hold');
  });
});

describe('a new ship’s arrival', () => {
  it('waits far out and high while the ship loads, then comes in', () => {
    const camera = new OrbitCamera();
    camera.frame([0, 0, 0], 100);
    const rest = camera.restDistance;
    camera.flyIn();
    expect(camera.state.arrival).toBe('waiting');
    expect(camera.distance).toBeGreaterThan(rest);
    expect(camera.pitch).toBeGreaterThan(HERO_POSE.pitch);
    const held = camera.distance;
    camera.update(1, 1000);
    expect(camera.distance).toBe(held);

    camera.land();
    expect(camera.state.arrival).toBe('landing');
    for (let frame = 0; frame < 600; frame++) camera.update(1 / 60, 1000 + frame * 16);
    expect(camera.distance).toBe(rest);
    expect(camera.pitch).toBe(HERO_POSE.pitch);
    expect(camera.state.arrival).toBe('none');
  });

  it('leaves the view alone for a viewer mid-drag', () => {
    const camera = new OrbitCamera();
    camera.frame([0, 0, 0], 100);
    camera.dragging = true;
    const before = { distance: camera.distance, pitch: camera.pitch };
    camera.flyIn();
    expect({ distance: camera.distance, pitch: camera.pitch }).toEqual(before);
    camera.land();
    expect(camera.state.arrival).toBe('none');
  });

  it('gives the pitch up to a pose', () => {
    const camera = new OrbitCamera();
    camera.flyIn();
    camera.land();
    camera.setPose(SIDE_POSE, 0);
    expect(camera.state.arrival).toBe('none');
  });

  it('does not outlive a pin (the zoom must ease again afterwards)', () => {
    const camera = new OrbitCamera();
    camera.frame([0, 0, 0], 100);
    camera.flyIn();
    camera.pin(500);
    expect(camera.state.arrival).toBe('none');
  });
});

describe('the lens', () => {
  it('holds the thumbnail’s angle while pinned', () => {
    const camera = new OrbitCamera();
    camera.setPose(SIDE_POSE, 0);
    camera.pin(500);
    expect(camera.state).toMatchObject({ motion: 'hold', lens: 'pinned' });
    expect({ yaw: camera.yaw, pitch: camera.pitch }).toEqual(THUMBNAIL_POSE);
  });

  it('shows the pinned frustum, then eases into the normal one and spins', () => {
    const camera = new OrbitCamera();
    camera.frame([0, 0, 0], 100);
    camera.pin(500);
    const frustum = { left: -0.2, right: 0.6, bottom: -0.3, top: 0.1 };
    camera.pinTo(frustum);
    const { fake, written } = rig();
    camera.apply(fake, 0);
    expect(written.offCenter?.map((edge) => +edge.toFixed(6))).toEqual([-0.2, 0.6, -0.3, 0.1]);
    expect(distanceOf(written.eye, written.target)).toBeCloseTo(500);

    camera.release(1000, 2000);
    expect(camera.state.lens).toBe('releasing');
    camera.apply(fake, 3000);
    const tangent = Math.tan(FIELD_OF_VIEW / 2);
    expect(written.offCenter?.[3]).toBeCloseTo(tangent);
    expect(distanceOf(written.eye, written.target)).toBeCloseTo(camera.restDistance);

    camera.update(0, 3000);
    expect(camera.state).toMatchObject({ lens: 'normal', motion: 'spin' });
  });

  it('shows the ship the normal way when a pin could not be measured', () => {
    const camera = new OrbitCamera();
    camera.pin(500);
    camera.release(1000, 0);
    expect(camera.state.lens).toBe('normal');
  });

  it('frames a square around the hull over the thumbnail’s place', () => {
    // A hull in the middle 20% of a 1000 px square first look, shown in a
    // 100 px thumbnail at the middle of a 1000 px wide canvas.
    const frustum = pinFrustum(
      { left: 400, right: 599, top: 400, bottom: 599 },
      { width: 1000, height: 1000 },
      { x: 450, y: 450, width: 100, height: 100 },
      { x: 0, y: 0, width: 1000, height: 1000 },
      1,
    );
    const tangent = Math.tan(FIELD_OF_VIEW / 2);
    // 100 px show 0.4 tangents, so the canvas's 1000 px show 4, centred.
    expect(frustum.left).toBeCloseTo(-2 * tangent);
    expect(frustum.right).toBeCloseTo(2 * tangent);
    expect(frustum.top).toBeCloseTo(2 * tangent);
    expect(frustum.bottom).toBeCloseTo(-2 * tangent);
  });
});

describe('looking', () => {
  it('frames a ship by its size, pulled back by the page’s framing', () => {
    const camera = new OrbitCamera();
    camera.frame([10, 0, 0], 100);
    camera.rest(camera.restDistance);
    const { fake, written } = rig();
    camera.apply(fake);
    expect(written.target).toEqual([10, 0, 0]);
    expect(written.fov).toBe(FIELD_OF_VIEW);
    const framed = distanceOf(written.eye, written.target);
    camera.setFraming(2, true);
    camera.apply(fake);
    expect(distanceOf(written.eye, written.target)).toBeCloseTo(framed * 2);
  });

  it('shifts the lens without moving the camera', () => {
    const camera = new OrbitCamera();
    const { fake, written } = rig();
    camera.apply(fake);
    const eye = [...written.eye];
    camera.lensShift = 0.2;
    camera.apply(fake);
    expect(written.eye).toEqual(eye);
    const [left, right] = written.offCenter ?? [0, 0];
    // What the camera looks at lands a fifth of the width right of centre.
    expect((0 - left) / (right - left)).toBeCloseTo(0.7);
  });

  it('puts everything back after a temporary look', () => {
    const camera = new OrbitCamera();
    const before = { yaw: camera.yaw, pitch: camera.pitch, fov: camera.fov, zoom: camera.zoom };
    const seen = camera.look({ fov: 0.1, zoom: 3 }, () => {
      camera.yaw = 2;
      camera.pitch = 1;
      return camera.fov;
    });
    expect(seen).toBe(0.1);
    expect({ yaw: camera.yaw, pitch: camera.pitch, fov: camera.fov, zoom: camera.zoom }).toEqual(
      before,
    );
  });

  it('keeps the wheel within its limits', () => {
    const camera = new OrbitCamera();
    camera.frame([0, 0, 0], 100);
    const framed = OrbitCamera.restFor(100);
    camera.wheel(100000, 100);
    expect(camera.restDistance).toBeCloseTo(framed * 3);
    camera.wheel(-100000, 100);
    expect(camera.restDistance).toBeCloseTo(framed * 0.4);
  });
});
