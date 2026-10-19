<script lang="ts">
  // A chapter's own ship (the liveries, the SKINRs): a full-bleed layer behind
  // the chapter's text with a glow and the faction's emblem, and the
  // section's own view of the ship over them, at half the chapter's height.
  // Beside the text (wide screens) the ship sits right of centre like the
  // stage's RIGHT placement.
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import SectionView from '#lib/components/section-view.svelte';
  import type { StagePlacement } from '#lib/stage.svelte.js';
  import type { CameraPose } from '#lib/viewer/poses.js';

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
</script>

<div class="section-stage pointer-events-none absolute inset-y-0" aria-hidden="true">
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
      scale={wide ? placement.scale : 1}
      class="absolute inset-0"
    />
  </div>
</div>

<style>
  /* Full-bleed: a viewport-wide band centred on the chapter. */
  .section-stage {
    left: 50%;
    width: 100vw;
    margin-left: -50vw;
    overflow: clip;
  }
  .frame {
    position: absolute;
    top: 50%;
    left: 0;
    width: 100vw;
    height: 100dvh;
    transform: translateY(-50%);
  }
  .emblem.wide {
    transform: translateX(var(--shift)) scale(var(--scale));
  }
</style>
