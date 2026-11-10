<script lang="ts">
  // A chapter's own ship (the liveries, the SKINRs): a full-bleed layer behind
  // the chapter's text with a glow and the faction's emblem, and the
  // section's own view of the ship over them, at half the chapter's height.
  // Beside the text (wide screens) the ship sits right of centre like the
  // stage's RIGHT placement; on a narrow screen it is a band above the text,
  // which would otherwise cover it (the chapter stacks, `flex-col`).
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import SectionView from '#lib/components/section-view.svelte';
  import type { StagePlacement } from '#lib/stage.svelte.js';
  import { SPHERE_SPAN, type CameraPose } from '#lib/viewer/poses.js';
  import { viewport } from '#lib/viewport.svelte.js';

  let {
    key,
    pose,
    tint,
    logoUrl,
    wide,
    placement,
  }: {
    /** The section view's name; each has its own canvas and camera. */
    key: string;
    pose: CameraPose;
    tint: string;
    logoUrl: string | null;
    /** Whether the chapter's text sits beside the ship. */
    wide: boolean;
    /** Where the ship sits beside the text. */
    placement: StagePlacement;
  } = $props();

  /** How much of the band's width the ship's sphere may fill. */
  const BAND_FILL = 0.85;
  /**
   * The view renders at the screen's size and is cropped to the band; framed
   * by the screen's height, a portrait phone would crop a long hull at the
   * sides. The camera pulls back until the ship fits the width (a smaller
   * scale moves it back).
   */
  const narrowScale = $derived(
    Math.min(1, (viewport.width / viewport.height) * (BAND_FILL / SPHERE_SPAN)),
  );
</script>

<div
  class="section-stage pointer-events-none relative h-[55svh] shrink-0 lg:absolute lg:inset-y-0 lg:h-auto"
  aria-hidden="true"
>
  <div
    class="absolute inset-0"
    style:background="radial-gradient(35% 40% at {wide ? 70 : 50}% 50%, color-mix(in oklab, {tint}
    14%, transparent), transparent 70%)"
  ></div>
  <div class="frame">
    {#if logoUrl}
      <!-- The emblem sits where the ship does: shifted and scaled like the
           stage's placement. -->
      <div
        class="emblem absolute inset-0"
        class:wide
        style:--shift="{placement.x * 100}vw"
        style:--scale={placement.scale}
      >
        <div
          class="absolute top-1/2 left-1/2 size-[min(120vh,105vw)] -translate-x-1/2 -translate-y-1/2"
        >
          <FactionEmblem url={logoUrl} {tint} class="size-full" />
        </div>
      </div>
    {/if}
    <!-- The canvas spans the whole width; the renderer shifts the ship right
         with an off-centre lens, so a ship sliding out to the left travels
         under the text to the screen edge. -->
    <SectionView
      {key}
      {pose}
      shift={wide ? placement.x : 0}
      scale={wide ? placement.scale : narrowScale}
      class="absolute inset-0"
    />
  </div>
</div>

<style>
  /* Full-bleed: a viewport-wide band (above the text on a narrow screen). */
  .section-stage {
    width: 100vw;
    margin-inline: calc(50% - 50vw);
    overflow: clip;
  }
  .frame {
    position: absolute;
    inset: 0;
    width: 100vw;
  }
  /* Behind the text, centred on the chapter, a screen tall. */
  @media (min-width: 64rem) {
    .section-stage {
      left: 50%;
      margin-inline: 0;
      margin-left: -50vw;
    }
    .frame {
      inset: auto;
      top: 50%;
      left: 0;
      height: 100svh;
      transform: translateY(-50%);
    }
  }
  .emblem.wide {
    transform: translateX(var(--shift)) scale(var(--scale));
  }
</style>
