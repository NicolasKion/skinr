<script lang="ts">
  // The livery section's own view of the ship: the renderer draws it a second
  // time from `pose` into renderer.liveryCanvas, hosted here, while the main
  // stage stays docked in the blueprint above. It renders only while near the
  // screen. Dragging it turns the ship.
  import { onMount } from 'svelte';
  import { getEngine } from '$lib/viewer/engine';
  import type { CameraPose } from '$lib/viewer/poses';
  import type { ShipRenderer } from '$lib/viewer/renderer';

  let {
    pose,
    shift = 0,
    scale = 1,
    class: className = '',
  }: {
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
      renderer?.setLiveryView(null);
    };
  });

  $effect(() => {
    if (!renderer || !host) return;
    const canvas = renderer.liveryCanvas;
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
    renderer?.setLiveryView({ pose, visible: live, shift, scale });
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
