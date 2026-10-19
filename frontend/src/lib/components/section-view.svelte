<script lang="ts">
  // A page section's own view of the ship (the liveries, the SKINRs): the
  // renderer draws the ship again from `pose` into the canvas of section
  // `key`, hosted here, while the main stage stays docked in the blueprint
  // above. It renders only while near the screen. Dragging it turns the ship.
  import { onMount } from 'svelte';
  import { getEngine } from '#lib/viewer/engine.js';
  import type { CameraPose } from '#lib/viewer/poses.js';
  import type { ShipRenderer } from '#lib/viewer/renderer.js';

  let {
    key,
    pose,
    shift = 0,
    scale = 1,
    class: className = '',
  }: {
    /** Which section view this is; each key has its own canvas and camera. */
    key: string;
    pose: CameraPose;
    /** How far right of centre the ship sits, as a share of the width. */
    shift?: number;
    /** The ship's size relative to filling the view. */
    scale?: number;
    class?: string;
  } = $props();

  /** How far outside the viewport the view keeps rendering. */
  const LIVE_MARGIN = '200px';

  let host = $state<HTMLElement>();
  let renderer = $state<ShipRenderer | null>(null);
  let live = $state(false);

  onMount(() => {
    let cancelled = false;
    getEngine()
      .then((engine) => {
        if (!cancelled) renderer = engine.renderer;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      renderer?.setSectionView(key, null);
    };
  });

  $effect(() => {
    if (!renderer || !host) return;
    const canvas = renderer.sectionCanvas(key);
    host.append(canvas);
    return () => {
      if (canvas.parentElement === host) canvas.remove();
    };
  });

  $effect(() => {
    if (!host) return;
    const observer = new IntersectionObserver(([entry]) => (live = entry.isIntersecting), {
      rootMargin: LIVE_MARGIN,
    });
    observer.observe(host);
    return () => observer.disconnect();
  });

  $effect(() => {
    renderer?.setSectionView(key, { pose, visible: live, shift, scale });
  });
</script>

<div class="livery-view pointer-events-auto {className}" bind:this={host}></div>

<style>
  .livery-view :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
    cursor: grab;
  }
  .livery-view :global(canvas:active) {
    cursor: grabbing;
  }
</style>
