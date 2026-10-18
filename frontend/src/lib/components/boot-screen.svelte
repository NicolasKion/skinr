<script lang="ts">
  // Covers the page on the first load, until the visit's first ship is loaded
  // and on its way in (stage.booted). Starting the engine and streaming the
  // first hull is the heaviest moment of a visit; with the page's own
  // animations running at the same time, everything stutters. So the page
  // waits behind this (app.html's data-booting holds its animations, see
  // layout.css), then everything starts at once, smoothly.
  import { fade } from 'svelte/transition';
  import Logo from '$lib/components/logo.svelte';
  import { stage } from '$lib/stage.svelte';

  /** How long the screen takes to lift, in ms. */
  const LIFT_MS = 700;

  /** What each step of the start says, and how far along the bar it is. */
  const STEPS = {
    starting: { label: 'Starting the engine', progress: 0.08 },
    index: { label: 'Reading the game client', progress: 0.2 },
    sof: { label: 'Loading ship designs', progress: 0.4 },
    gpu: { label: 'Starting the renderer', progress: 0.55 },
    ready: { label: 'Assembling', progress: 0.75 },
  };

  const step = $derived(STEPS[stage.engineStage ?? 'starting']);
  const label = $derived(
    stage.engineStage === 'ready' && stage.ship ? `Assembling the ${stage.ship.name}` : step.label,
  );

  $effect(() => {
    if (stage.booted) delete document.documentElement.dataset.booting;
  });
</script>

{#if !stage.booted}
  <div class="boot fixed inset-0 z-50 grid place-items-center" out:fade={{ duration: LIFT_MS }}>
    <div class="flex w-64 flex-col items-center gap-6">
      <Logo class="mark size-12 text-foreground" />
      <div class="h-px w-full overflow-hidden bg-foreground/10">
        <div class="bar h-full bg-foreground/60" style:transform="scaleX({step.progress})"></div>
      </div>
      <p class="text-xs text-muted-foreground" aria-live="polite">{label}</p>
    </div>
  </div>
{/if}

<style>
  .boot {
    background: var(--background);
  }
  .bar {
    transform-origin: left;
    transition: transform 1.2s var(--ease-apple);
  }
  /* A slow breath, so a long start still looks alive. Exempt from the hold
     on the page's animations. */
  .boot :global(.mark) {
    animation: breathe 2.4s ease-in-out infinite;
    animation-play-state: running !important;
  }
  @keyframes breathe {
    50% {
      opacity: 0.45;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .boot :global(.mark) {
      animation: none;
    }
  }
</style>
