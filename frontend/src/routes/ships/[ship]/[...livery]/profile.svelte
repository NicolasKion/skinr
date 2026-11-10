<script lang="ts">
  // The profile chapter: who builds the ship and its lore, beside the ship
  // seen broadside.
  import EveText from '#lib/components/eve-text.svelte';
  import type { ShipDetail } from '#lib/types.js';
  import Chapter from './chapter.svelte';

  let {
    ship,
    tint,
    onenter,
    text = $bindable(),
  }: {
    ship: ShipDetail;
    tint: string;
    onenter: () => void;
    /** The text block's element. */
    text?: HTMLElement;
  } = $props();
</script>

<Chapter id="profile" {onenter} bind:text>
  {#snippet stage()}
    <!-- On a narrow screen the ship sits here, above the text. -->
    <div class="h-[40svh] w-full shrink-0 lg:hidden" aria-hidden="true"></div>
  {/snippet}
  <p class="reveal eyebrow mb-4" style:color={tint}>Profile</p>
  <h2 class="reveal display mb-8 text-[clamp(2.25rem,4vw,3.5rem)]">
    <span class="text-sheen">{ship.faction?.name ?? ship.race?.name ?? 'Unaffiliated'}</span>
    <span class="block text-muted-foreground">{ship.group.name}</span>
  </h2>
  <div class="reveal description text-[1.0625rem] leading-[1.7] text-foreground/75">
    <EveText segments={ship.description_rich} />
  </div>
</Chapter>

<style>
  /* Long lore reads better in short paragraphs than in one wall. */
  .description :global(br + br) {
    display: block;
    content: '';
    margin-top: 0.9em;
  }
</style>
