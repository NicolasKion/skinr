<script lang="ts">
  // Behind the stage: a soft spotlight and the faction's emblem as tinted
  // light. Fixed and at z-0, so the transparent canvas shows it. It moves
  // with the ship (the stage's placement) and leaves once the ship docks in
  // the blueprint. Kept quiet on purpose; the ship is the subject.
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import { stage, type StagePlacement } from '#lib/stage.svelte.js';

  let {
    tint,
    logoUrl,
    placement,
    docked,
  }: {
    tint: string;
    logoUrl: string | null;
    placement: StagePlacement;
    docked: boolean;
  } = $props();
</script>

<div
  class="backdrop pointer-events-none fixed inset-0 z-0 overflow-hidden"
  style:opacity={docked ? 0 : 1 - stage.dim}
  style:transform="translate({placement.x * 100}vw, {(placement.y ?? 0) * 100}svh) scale({placement.scale})"
>
  <div
    class="absolute inset-0"
    style:background="radial-gradient(45% 40% at 50% 52%, color-mix(in oklab, {tint} 14%, transparent),
    transparent 70%)"
  ></div>
  {#if logoUrl}
    {#key logoUrl}
      <div
        class="emblem-motion absolute top-1/2 left-1/2 size-[min(120vh,105vw)] -translate-x-1/2 -translate-y-1/2"
        style:opacity={1 - stage.hologram}
      >
        <FactionEmblem url={logoUrl} {tint} class="emblem-in size-full" />
      </div>
    {/key}
  {/if}
</div>

<style>
  .backdrop {
    transition:
      transform 0.9s cubic-bezier(0.65, 0, 0.35, 1),
      opacity 0.9s var(--ease-apple);
  }
  .emblem-motion {
    transition: opacity 0.9s var(--ease-apple);
  }
  .emblem-motion :global(.emblem-in) {
    animation: emblem-in 2.4s var(--ease-out-expo) both;
  }
  @keyframes emblem-in {
    from {
      opacity: 0;
      transform: scale(1.12);
      filter: blur(12px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .emblem-motion :global(.emblem-in) {
      animation: none;
    }
  }
</style>
