<script lang="ts">
  // The shared 3D stage. The canvas always covers the whole viewport and is
  // never resized for a mode change: the dock is the same canvas scaled down
  // and masked with a rounded clip, both GPU-composited, so moving between
  // hero and dock is one smooth transform rather than a re-render at every size.
  import { onMount, untrack } from 'svelte';
  import { stage, type StagePlacement } from '#lib/stage.svelte.js';
  import type { Engine, ShowTransition } from '#lib/viewer/engine.js';
  import { HERO_POSE, SPHERE_SPAN, TRANSITION_MS, easeInOut } from '#lib/viewer/poses.js';
  import { preloadShip } from '#lib/viewer/resources.js';

  /**
   * How much of the dock's visible width or height a ship's bounding sphere
   * may fill: the dock zooms in on the canvas no further, so a hull always
   * fits, whatever the dock's shape, with some room around it.
   */
  const DOCK_FILL = 0.85;
  /** How far a dimmed stage fades. */
  const DIM_OPACITY = 0.85;
  /** Lift the boot screen after this long, in ms, whatever is still loading. */
  const BOOT_TIMEOUT = 30000;

  let host: HTMLDivElement;
  let engine = $state<Engine | null>(null);
  let viewport = $state({ width: 1, height: 1 });

  onMount(() => {
    let alive = true;
    stage.status = 'starting';
    const giveUp = setTimeout(() => (stage.booted = true), BOOT_TIMEOUT);
    import('#lib/viewer/engine.js')
      .then(({ getEngine }) => getEngine((step) => (stage.engineStage = step)))
      .then((ready) => {
        if (!alive) return;
        // A page without a ship has nothing more to wait for.
        if (!stage.dna) stage.booted = true;
        host.append(ready.renderer.canvas);
        ready.renderer.onMissing = (path, error) => {
          console.warn(`Missing resource ${path}: ${error.message}`);
          if (!stage.missing.includes(path)) stage.missing = [...stage.missing, path];
        };
        stage.resetView = () => ready.renderer.resetView();
        stage.releasePin = (duration) => ready.renderer.releasePin(duration);
        engine = ready;
      })
      .catch((error: Error) => {
        stage.status = 'error';
        stage.error = error.message;
        stage.booted = true;
      });
    return () => {
      alive = false;
      clearTimeout(giveUp);
    };
  });

  // A picked ship's files start downloading at once, even before the engine
  // is up (resources.ts).
  $effect(() => {
    const id = stage.ship?.id;
    if (id !== undefined) void preloadShip(id);
  });

  /**
   * The DNA whose ship has loaded (or failed to): a stage that appears
   * waits for this to catch up before it shows anything.
   */
  let loadedDna = $state<string | null>(null);

  // The ship shown last: a new DNA for the same ship is a livery change and
  // slides in as a carousel; a different ship flies in.
  let shownShipId: number | null = null;

  $effect(() => {
    const current = engine;
    const dna = stage.dna;
    const shipId = stage.ship?.id ?? null;
    if (!current || !dna) return;
    const ship = stage.ship;
    const { leap, slideShips } = untrack(() => ({
      leap: stage.leap,
      slideShips: stage.slideShips,
    }));
    const arrive = () => {
      if (stage.dna === dna) stage.onScreen = ship;
    };
    const slides =
      shipId !== null && shownShipId !== null && (shipId === shownShipId || slideShips);
    const transition: ShowTransition =
      leap && !leap.shown
        ? { kind: 'pin', rect: leap.rect }
        : slides
          ? { kind: 'slide', direction: stage.direction, onGlide: arrive }
          : { kind: 'fly' };
    shownShipId = shipId;
    stage.status = 'loading';
    stage.error = null;
    stage.missing = [];
    if (stage.pattern) current.registerPattern(stage.pattern.name, stage.pattern.record);
    current
      .show(dna, transition)
      .then((shown) => {
        if (shown && stage.dna === dna) {
          loadedDna = dna;
          stage.status = 'ready';
          stage.booted = true;
          arrive();
        }
      })
      .catch((error: Error) => {
        if (stage.dna !== dna) return;
        loadedDna = dna;
        arrive();
        stage.status = 'error';
        stage.error = error.message;
        stage.booted = true;
      });
  });

  /** Where the canvas sits: a translate and scale, and the clip around it. */
  interface Geometry {
    tx: number;
    ty: number;
    scale: number;
    /** How much farther the camera sits so the ship fits the dock (1: as framed). */
    frame: number;
    top: number;
    right: number;
    bottom: number;
    left: number;
    radius: number;
  }

  const FULL: Geometry = {
    tx: 0,
    ty: 0,
    scale: 1,
    frame: 1,
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    radius: 0,
  };

  /**
   * Where the canvas belongs: full screen, or scaled into the dock and clipped
   * to it. `inPage` gives the dock's page position instead of its viewport
   * position.
   */
  function place(inPage: boolean): Geometry {
    const { width: vw, height: vh } = viewport;
    const found = stage.dock;
    if (leaping || stage.mode !== 'dock' || !found) {
      // Full screen; a page's placement may still pull the camera back.
      const framing = stage.mode === 'hero' && !leaping ? (stage.placement.framing ?? 1) : 1;
      return framing === 1 ? FULL : { ...FULL, frame: framing };
    }
    const dock = inPage ? { ...found, x: found.pageX, y: found.pageY } : found;
    // A dock may run past the viewport's right edge (the explorer's frame
    // does, to hide its right corners): the ship centres on, and fits, the
    // part that is on screen.
    const shown = Math.max(
      1,
      Math.min(found.width, document.documentElement.clientWidth - found.x),
    );
    // The canvas always covers the dock, so nothing it draws (a ship zoomed
    // in) stops short of the dock's edge. The blueprint frames the hull to its
    // slot itself; other docks have the camera pull back until the ship's
    // sphere fits the part of the dock that is on screen.
    const cover = Math.max(dock.width / vw, dock.height / vh);
    const fitted = stage.blueprintFit !== null;
    const sphere = vh * SPHERE_SPAN;
    const fit = Math.min((shown * DOCK_FILL) / sphere, (dock.height * DOCK_FILL) / sphere);
    const scale = cover;
    const frame = fitted ? 1 : Math.max(1, cover / fit);
    const centerX = dock.x + (fitted ? dock.width : shown) / 2;
    const tx = centerX - (vw * scale) / 2;
    const ty = dock.y + dock.height / 2 - (vh * scale) / 2;
    return {
      tx,
      ty,
      scale,
      frame,
      top: (dock.y - ty) / scale,
      left: (dock.x - tx) / scale,
      right: vw - (dock.x + dock.width - tx) / scale,
      bottom: vh - (dock.y + dock.height - ty) / scale,
      radius: fitted ? 0 : stage.dockRadius / scale,
    };
  }
  /** During a leap the stage covers the viewport as is, whatever the page wants. */
  const leaping = $derived(stage.leap !== null);
  const target = $derived(place(false));
  const targetInPage = $derived(place(true));

  /**
   * The geometry on screen, updated every frame. Moving to or from the hero
   * glides for TRANSITION_MS, blending toward where the target is on each
   * frame (so it keeps up with scrolling) and landing exactly on it; after
   * that there is no easing at all. Settled in a dock that scrolls, the stage
   * is placed in the page rather than over the viewport, so the browser
   * scrolls it along with everything else: a fixed element repositioned from
   * script always trails the scroll by a frame. A fixed dock keeps it fixed.
   */
  let shown = $state<Geometry>(FULL);
  let anchored = $state(false);
  let move: { from: Geometry; start: number } | null = null;
  /** Whether a glide is under way (`move`, for the framing to follow). */
  let gliding = $state(false);

  const same = (a: Geometry, b: Geometry) =>
    a.tx === b.tx &&
    a.ty === b.ty &&
    a.scale === b.scale &&
    a.frame === b.frame &&
    a.top === b.top &&
    a.right === b.right &&
    a.bottom === b.bottom &&
    a.left === b.left &&
    a.radius === b.radius;

  const blend = (from: Geometry, to: Geometry, k: number): Geometry => ({
    tx: from.tx + (to.tx - from.tx) * k,
    ty: from.ty + (to.ty - from.ty) * k,
    scale: from.scale + (to.scale - from.scale) * k,
    frame: from.frame + (to.frame - from.frame) * k,
    top: from.top + (to.top - from.top) * k,
    right: from.right + (to.right - from.right) * k,
    bottom: from.bottom + (to.bottom - from.bottom) * k,
    left: from.left + (to.left - from.left) * k,
    radius: from.radius + (to.radius - from.radius) * k,
  });

  // A change of place glides only when the hero is one end of it; the swap
  // between the two docks happens off screen and is instant.
  const placeKey = $derived(`${stage.mode}:${stage.dockName}:${leaping}`);
  let lastMode = stage.mode;
  $effect(() => {
    void placeKey;
    const mode = stage.mode;
    untrack(() => {
      // A stage on its way out fades where it is; it does not glide.
      const glides =
        !leaping && visible && (lastMode === 'hero' || mode === 'hero') && lastMode !== mode;
      lastMode = mode;
      const from = anchored
        ? { ...shown, tx: shown.tx - window.scrollX, ty: shown.ty - window.scrollY }
        : { ...shown };
      anchored = false;
      move = glides ? { from, start: performance.now() } : null;
      gliding = glides;
    });
  });

  onMount(() => {
    let request = 0;
    const tick = (now: number) => {
      request = requestAnimationFrame(tick);
      let next: Geometry;
      if (leaping) {
        next = FULL;
        anchored = false;
      } else if (move) {
        const t = Math.min(1, (now - move.start) / TRANSITION_MS);
        next = blend(move.from, target, easeInOut(t));
        if (t >= 1) {
          move = null;
          gliding = false;
        }
        anchored = false;
      } else if (!visible) {
        // Fading out: stay exactly where it was (still scrolling with the
        // page if it was docked) rather than jumping somewhere.
        return;
      } else {
        // A dock that scrolls with the page anchors the stage in it; a fixed
        // one keeps it fixed, so nothing has to follow the scroll at all.
        anchored = stage.mode === 'dock' && stage.dock !== null && !stage.dock.fixed;
        next = anchored ? targetInPage : target;
      }
      if (!same(next, shown)) shown = next;
    };
    request = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(request);
  });

  const transform = $derived(`translate(${shown.tx}px, ${shown.ty}px) scale(${shown.scale})`);
  const clip = $derived(
    `inset(${shown.top}px ${shown.right}px ${shown.bottom}px ${shown.left}px round ${shown.radius}px)`,
  );

  /**
   * Whether placement changes animate. A stage nobody sees (the explorer on a
   * phone, which has no panel) places itself at once, and keeps doing so for
   * a frame after it shows, so it appears where it belongs rather than
   * gliding there from wherever it was left.
   */
  let settled = $state(false);
  $effect(() => {
    // Only a stage out of sight for a whole frame counts as gone: leaving the
    // explorer for a ship's page drops its dock a moment before the page asks
    // for the hero, and the ship must glide out of the dock, not jump.
    const now = visible;
    const frame = requestAnimationFrame(() => (settled = now));
    return () => cancelAnimationFrame(frame);
  });

  /** Whether the page wants the stage seen. */
  const wanted = $derived(
    stage.leap
      ? stage.leap.shown
      : stage.mode === 'hero' || (stage.mode === 'dock' && stage.dock !== null),
  );
  /**
   * A stage that appears (on a ship's page after a phone's explorer, which
   * has none) stays out of sight until its ship has loaded: meanwhile the
   * canvas holds a half-streamed hull, or the previous one. Once shown it
   * stays shown, and later ships and liveries arrive the usual way.
   */
  let waiting = $state(false);
  let wasWanted = false;
  $effect(() => {
    const now = wanted;
    const pending = !leaping && stage.dna !== null && loadedDna !== stage.dna;
    untrack(() => {
      if (now && !wasWanted && pending) waiting = true;
      else if (!now || !pending) waiting = false;
      wasWanted = now;
    });
  });
  const visible = $derived(wanted && !waiting);
  /**
   * The hero's shift beside the page, its dimming and the camera's pull-back
   * (only the hero moves beside the page; the dock has its own frame). A
   * stage on its way out keeps them as they were and only fades: a ship
   * leaving the hero for a dock that is not there (the explorer on a phone)
   * must not slide or grow while it goes.
   */
  let held = $state({ placement: { x: 0, scale: 1 } as StagePlacement, dim: 0, frame: 1 });
  $effect(() => {
    if (!visible && !leaping) return;
    const hero = stage.mode === 'hero' && !leaping;
    held = {
      placement: hero ? stage.placement : { x: 0, scale: 1 },
      dim: hero ? stage.dim : 0,
      frame: target.frame,
    };
  });
  const placement = $derived(held.placement);
  const dim = $derived(held.dim);

  // A leap lands on the hero's framing; set again only when it changes, so
  // the end of a leap does not interrupt the spin.
  const pose = $derived(leaping ? HERO_POSE : stage.pose);
  $effect(() => {
    engine?.renderer.setPose(pose);
  });

  $effect(() => {
    engine?.renderer.setHologram(leaping ? 0 : stage.hologram);
  });

  // A broken ship shows as a broken signal.
  $effect(() => {
    engine?.renderer.setGlitch(stage.broken ? 1 : 0);
  });

  /**
   * The blueprint framing: half the canvas's height in metres, so that once
   * scaled into the slot the slot's width spans the hull's length. Only the
   * slot's size matters, so scrolling does not change it.
   */
  const blueprintHalfHeight = $derived.by(() => {
    const fit = stage.blueprintFit;
    const dock = stage.dock;
    if (!fit || stage.mode !== 'dock' || !dock) return null;
    const { width: vw, height: vh } = viewport;
    const scale = Math.max(dock.width / vw, dock.height / vh);
    const pixelsPerMetre = dock.width / (fit.halfLength * 2);
    // The canvas's height on screen is vh * scale CSS pixels.
    return (vh * scale) / (2 * pixelsPerMetre);
  });
  $effect(() => {
    engine?.renderer.setBlueprint(leaping ? null : blueprintHalfHeight);
  });
  // In a dock the camera pulls back so the ship fits the dock (place).
  // During a glide it moves in step with the clip, or the ship would outgrow
  // the dock's outline on its way out; a stage appearing frames its ship at
  // once, like it places it.
  $effect(() => {
    if (gliding) engine?.renderer.setFraming(shown.frame, true);
    else
      engine?.renderer.setFraming(
        held.frame,
        untrack(() => !settled),
      );
  });

  // The wheel zooms in a preview fixed to the screen (the explorer's and the
  // SKINR pages' panel); where the page scrolls (a ship's page) it needs ctrl.
  $effect(() => {
    engine?.renderer.setWheelZoom(stage.mode === 'dock' && stage.dock?.fixed === true);
  });

  // Draw the blueprint once the ship (in this livery) is fully on stage.
  let drafting: string | null = null;
  $effect(() => {
    const dna = stage.dna;
    // Not mid-leap: the pinned camera would draw it.
    if (!engine || !stage.wantBlueprint || stage.status !== 'ready' || !dna || leaping) return;
    if (stage.blueprint?.dna === dna || drafting === dna) return;
    drafting = dna;
    engine.renderer
      .captureBlueprint()
      .then((drawing) => {
        if (drawing && stage.dna === dna) stage.blueprint = { dna, drawing };
      })
      .catch((error: Error) => console.warn('Could not draw the blueprint', error))
      .finally(() => {
        if (drafting === dna) drafting = null;
      });
  });
</script>

<svelte:window bind:innerWidth={viewport.width} bind:innerHeight={viewport.height} />

<div
  class="stage top-0 left-0 z-10 h-dvh w-screen origin-top-left {anchored ? 'absolute' : 'fixed'}"
  class:leaping
  class:unsettled={!settled}
  class:gone={!visible}
  class:pointer-events-none={!visible || leaping}
  class:pointer-events-auto={visible && !leaping}
  style:transform
  style:clip-path={clip}
  style:opacity={visible ? 1 : 0}
  aria-hidden="true"
>
  <div
    bind:this={host}
    class="canvas-host h-full w-full"
    style:transform="translate({placement.x * 100}vw, {(placement.y ?? 0) * 100}dvh) scale({placement.scale})"
    style:opacity={1 - dim * DIM_OPACITY}
  ></div>
</div>

<style>
  .stage {
    view-transition-name: stage;
    transition: opacity 0.7s var(--ease-apple);
  }
  /* Going away is quick; only showing up takes its time. */
  .stage.gone {
    transition-duration: 0.25s;
  }
  .stage {
    will-change: transform, clip-path;
  }
  /* A leap shows and places the stage at once; so does a stage appearing. */
  .stage.leaping,
  .stage.leaping .canvas-host,
  .stage.unsettled .canvas-host {
    transition: none;
  }
  /* The hero's shift beside a chapter's text: same length and curve as
     every other place transition. */
  .canvas-host {
    transition:
      transform 0.9s cubic-bezier(0.65, 0, 0.35, 1),
      opacity 0.9s var(--ease-apple);
  }
  .canvas-host :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
    cursor: grab;
  }
  .canvas-host :global(canvas:active) {
    cursor: grabbing;
  }
</style>
