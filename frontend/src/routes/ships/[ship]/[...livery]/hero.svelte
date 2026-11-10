<script lang="ts">
  // The ship page's first screen: the ship's name over the stage, the livery
  // picker, and what the stage is doing while it loads. Everything in it
  // lets pointer events through to the stage except its controls.
  import { ChevronDown, LoaderCircle } from '@lucide/svelte';
  import { page } from '$app/state';
  import StageAlert from '#lib/components/stage-alert.svelte';
  import ThumbnailDebug from '#lib/components/thumbnail-debug.svelte';
  import { firstSentence } from '#lib/eve.js';
  import { stage } from '#lib/stage.svelte.js';
  import type { ShipDetail } from '#lib/types.js';
  import { whenCrossing } from './chapters';
  import LiveryPicker from './livery-picker.svelte';
  import type { LiveryState } from './livery.svelte';
  import { NARROW_CLEAR_CSS } from './narrow';

  let {
    ship,
    tint,
    builder,
    livery,
    card,
    onenter,
    text = $bindable(),
  }: {
    ship: ShipDetail;
    tint: string;
    /** Who builds the ship, for the line over its name. */
    builder: string;
    livery: LiveryState;
    /** The page's share card (the debug tools show it). */
    card: string;
    /** Called when the hero comes to cross the middle of the screen. */
    onenter: () => void;
    /** The text block's element. */
    text?: HTMLElement;
  } = $props();

  const loading = $derived(stage.status === 'starting' || stage.status === 'loading');
</script>

<section
  class="pointer-events-none relative z-20 flex min-h-svh flex-col pt-(--ship-clear) lg:h-svh lg:pt-0"
  style:--tint={tint}
  style:--ship-clear={NARROW_CLEAR_CSS}
  use:whenCrossing={onenter}
>
  <div
    bind:this={text}
    class="mx-auto mt-auto flex w-full max-w-[90rem] flex-wrap items-end justify-between gap-8 px-6 pb-16"
  >
    <div class="intro max-w-xl">
      <p class="eyebrow mb-3" style:color={tint}>
        {[builder, ship.group.name, ship.meta_group?.name].filter(Boolean).join(' · ')}
      </p>
      <h1 class="display text-[clamp(3.25rem,7.5vw,6.5rem)]" style:view-transition-name="ship-name">
        <span class="text-tint-sheen">{ship.name}</span>
      </h1>
      {#if ship.description}
        <p class="mt-4 text-lg leading-relaxed text-foreground/70">
          {firstSentence(ship.description)}
        </p>
      {/if}
    </div>

    <div class="intro intro-late pointer-events-auto flex max-w-full flex-col items-end gap-3">
      <div class="flex items-center gap-2">
        <p class="text-sm text-foreground/80">
          <kbd class="mr-2 hidden text-[0.7rem] text-muted-foreground/70 lg:inline">← →</kbd>
          <span class="text-muted-foreground">Skin</span>
          <span class="mx-1.5 text-foreground/20">/</span>{livery.name}
        </p>
        {#if page.data.debug}
          <ThumbnailDebug livery={livery.onShip} {card} />
        {/if}
      </div>
      <LiveryPicker
        options={livery.options}
        selected={livery.skin?.id ?? null}
        onselect={(id) => livery.pickSkin(id)}
      />
    </div>
  </div>

  <!-- A broken ship glitches; this says why. -->
  <div class="pointer-events-auto absolute top-32 right-6 lg:right-10">
    <StageAlert name={ship.name} />
  </div>

  <div class="absolute inset-x-0 bottom-5 flex justify-center">
    {#if loading}
      <span class="flex items-center gap-2 text-xs text-muted-foreground">
        <LoaderCircle class="size-3.5 animate-spin" />
        {stage.status === 'starting' ? 'Starting the engine' : `Assembling the ${ship.name}`}
      </span>
    {:else if stage.status === 'error'}
      <!-- The alert above says what went wrong. -->
    {:else}
      <span class="cue flex flex-col items-center text-[0.7rem] text-muted-foreground">
        Scroll <ChevronDown class="size-4" />
      </span>
    {/if}
  </div>
</section>

<style>
  .intro {
    animation: intro 1.1s var(--ease-out-expo) 0.25s both;
  }
  .intro-late {
    animation-delay: 0.45s;
  }
  .cue {
    animation: cue 2.4s ease-in-out infinite;
  }
  @keyframes intro {
    from {
      opacity: 0;
      transform: translateY(24px);
      filter: blur(6px);
    }
  }
  @keyframes cue {
    0%,
    100% {
      transform: translateY(0);
      opacity: 0.6;
    }
    50% {
      transform: translateY(5px);
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .intro,
    .cue {
      animation: none;
    }
  }
</style>
