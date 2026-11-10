<script lang="ts">
  // A chapter below the fold: a full screen whose text takes the left part
  // while the ship fills the rest (the main stage, or the chapter's own view
  // given as `stage`). On a narrow screen the two stack, the ship above.
  import type { Snippet } from 'svelte';
  import { whenCrossing } from './chapters';

  let {
    id,
    onenter,
    stage,
    children,
    text = $bindable(),
    class: className = '',
  }: {
    id: string;
    /** Called when the chapter comes to cross the middle of the screen. */
    onenter: () => void;
    /** What sits beside (or, narrow, above) the text: the chapter's own ship, or room for the main one. */
    stage?: Snippet;
    children: Snippet;
    /** The text block's element. */
    text?: HTMLElement;
    class?: string;
  } = $props();
</script>

<section {id} class="chapter flex-col lg:flex-row {className}" use:whenCrossing={onenter}>
  {@render stage?.()}
  <div class="chapter-text relative" bind:this={text}>
    {@render children()}
  </div>
</section>

<style>
  .chapter {
    display: flex;
    align-items: center;
    min-height: 100svh;
    max-width: 90rem;
    margin-inline: auto;
    padding: 6rem 1.5rem;
  }
  .chapter-text {
    width: 100%;
    max-width: 36rem;
  }
  @media (min-width: 64rem) {
    .chapter {
      padding-inline: 3rem;
    }
    .chapter-text {
      width: 42%;
    }
  }
</style>
