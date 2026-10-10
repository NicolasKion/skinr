<script lang="ts">
  // The shared 3D stage. The canvas always covers the whole viewport and is
  // never resized for a mode change: the dock is the same canvas scaled down
  // and masked with a rounded clip, both GPU-composited, so moving between
  // hero and dock is one smooth transform rather than a re-render at every size.
  import { onMount, untrack } from 'svelte';
  import { stage } from '$lib/stage.svelte';
  import type { Engine, ShowTransition } from '$lib/viewer/engine';
  import { TRANSITION_MS, easeInOut } from '$lib/viewer/poses';

  /**
   * How far the dock zooms in on the canvas, in dock widths per canvas width.
   * Covering the dock outright would crop long hulls on a landscape canvas;
   * this keeps a ship at about three quarters of the dock's width.
   */
  const DOCK_ZOOM = 1.7;
  /** How far a dimmed stage fades. */
  const DIM_OPACITY = 0.85;

  let host: HTMLDivElement;
  let engine = $state<Engine | null>(null);
  let viewport = $state({ width: 1, height: 1 });

  onMount(() => {
    let alive = true;
    stage.status = 'starting';
    import('$lib/viewer/engine')
      .then(({ getEngine }) => getEngine())
      .then((ready) => {
        if (!alive) return;
        host.append(ready.renderer.canvas);
        ready.renderer.onMissing = (path, error) => {
          console.warn(`Missing resource ${path}: ${error.message}`);
          if (!stage.missing.includes(path)) stage.missing = [...stage.missing, path];
        };
        stage.resetView = () => ready.renderer.resetView();
        engine = ready;
      })
      .catch((error: Error) => {
        stage.status = 'error';
        stage.error = error.message;
      });
    return () => {
      alive = false;
    };
  });

  // The ship shown last: a new DNA for the same ship is a livery change and
  // slides in as a carousel; a different ship flies in.
  let shownShipId: number | null = null;

  $effect(() => {
    const current = engine;
    const dna = stage.dna;
    const shipId = stage.ship?.id ?? null;
    if (!current || !dna) return;
    const transition: ShowTransition =
      shipId !== null && shipId === shownShipId
        ? { kind: 'slide', direction: stage.direction }
        : { kind: 'fly' };
    shownShipId = shipId;
    stage.status = 'loading';
    stage.error = null;
    stage.missing = [];
    if (stage.pattern) current.registerPattern(stage.pattern.name, stage.pattern.record);
    current
      .show(dna, transition)
      .then((shown) => {
        if (shown && stage.dna === dna) stage.status = 'ready';
      })
      .catch((error: Error) => {
        if (stage.dna !== dna) return;
        stage.status = 'error';
        stage.error = error.message;
      });
  });

  /** Where the canvas sits: a translate and scale, and the clip around it. */
  interface Geometry {
    tx: number;
    ty: number;
    scale: number;
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
    if (stage.mode !== 'dock' || !found) return FULL;
    const dock = inPage ? { ...found, x: found.pageX, y: found.pageY } : found;
    // The blueprint covers its slot exactly (the camera frames the hull to
    // it); other docks zoom in only so far.
    const cover = Math.max(dock.width / vw, dock.height / vh);
    const fitted = stage.blueprintFit !== null;
    const scale = fitted ? cover : Math.min(cover, (dock.width / vw) * DOCK_ZOOM);
    const tx = dock.x + dock.width / 2 - (vw * scale) / 2;
    const ty = dock.y + dock.height / 2 - (vh * scale) / 2;
    return {
      tx,
      ty,
      scale,
      top: (dock.y - ty) / scale,
      left: (dock.x - tx) / scale,
      right: vw - (dock.x + dock.width - tx) / scale,
      bottom: vh - (dock.y + dock.height - ty) / scale,
      radius: fitted ? 0 : stage.dockRadius / scale,
    };
  }
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

  const same = (a: Geometry, b: Geometry) =>
    a.tx === b.tx &&
    a.ty === b.ty &&
    a.scale === b.scale &&
    a.top === b.top &&
    a.right === b.right &&
    a.bottom === b.bottom &&
    a.left === b.left &&
    a.radius === b.radius;

  const blend = (from: Geometry, to: Geometry, k: number): Geometry => ({
    tx: from.tx + (to.tx - from.tx) * k,
    ty: from.ty + (to.ty - from.ty) * k,
    scale: from.scale + (to.scale - from.scale) * k,
    top: from.top + (to.top - from.top) * k,
    right: from.right + (to.right - from.right) * k,
    bottom: from.bottom + (to.bottom - from.bottom) * k,
    left: from.left + (to.left - from.left) * k,
    radius: from.radius + (to.radius - from.radius) * k,
  });

  // A change of place glides only when the hero is one end of it; the swap
  // between the two docks happens off screen and is instant.
  const placeKey = $derived(`${stage.mode}:${stage.dockName}`);
  let lastMode = stage.mode;
  $effect(() => {
    void placeKey;
    const mode = stage.mode;
    untrack(() => {
      const glides = (lastMode === 'hero' || mode === 'hero') && lastMode !== mode;
      lastMode = mode;
      const from = anchored
        ? { ...shown, tx: shown.tx - window.scrollX, ty: shown.ty - window.scrollY }
        : { ...shown };
      anchored = false;
      move = glides ? { from, start: performance.now() } : null;
    });
  });

  onMount(() => {
    let request = 0;
    const tick = (now: number) => {
      request = requestAnimationFrame(tick);
      let next: Geometry;
      if (move) {
        const t = Math.min(1, (now - move.start) / TRANSITION_MS);
        next = blend(move.from, target, easeInOut(t));
        if (t >= 1) move = null;
        anchored = false;
      } else if (stage.mode === 'hidden') {
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

  const visible = $derived(stage.mode === 'hero' || (stage.mode === 'dock' && stage.dock !== null));
  // Only the hero moves beside the page; the dock has its own frame.
  const placement = $derived(stage.mode === 'hero' ? stage.placement : { x: 0, scale: 1 });
  const dim = $derived(stage.mode === 'hero' ? stage.dim : 0);

  $effect(() => {
    engine?.renderer.setPose(stage.pose);
  });

  $effect(() => {
    engine?.renderer.setHologram(stage.hologram);
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
    engine?.renderer.setBlueprint(blueprintHalfHeight);
  });

  // Draw the blueprint once the ship (in this livery) is fully on stage.
  let drafting: string | null = null;
  $effect(() => {
    const dna = stage.dna;
    if (!engine || !stage.wantBlueprint || stage.status !== 'ready' || !dna) return;
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
  class:pointer-events-none={!visible}
  class:pointer-events-auto={visible}
  style:transform
  style:clip-path={clip}
  style:opacity={visible ? 1 : 0}
  aria-hidden="true"
>
  <div
    bind:this={host}
    class="canvas-host h-full w-full"
    style:transform="translateX({placement.x * 100}vw) scale({placement.scale})"
    style:opacity={1 - dim * DIM_OPACITY}
  ></div>
</div>

<style>
  .stage {
    view-transition-name: stage;
    transition: opacity 0.7s var(--ease-apple);
    will-change: transform, clip-path;
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
