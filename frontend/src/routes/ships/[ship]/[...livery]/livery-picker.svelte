<script lang="ts">
  // The hero's livery picker, a carousel: its track slides so the selected
  // livery sits in the middle, however many there are, and the ends fade out.
  import SkinIcon from '#lib/components/skin-icon.svelte';
  import type { LiveryOption } from './livery.svelte';

  let {
    options,
    selected,
    onselect,
  }: {
    options: LiveryOption[];
    /** The selected livery's id; null is the hull's own colors. */
    selected: number | null;
    onselect: (id: number | null) => void;
  } = $props();

  let track = $state<HTMLDivElement>();
  let width = $state(0);
  const shift = $derived.by(() => {
    const index = options.findIndex((option) => option.id === selected);
    const chosen = track?.children[index] as HTMLElement | undefined;
    if (!chosen || !width) return 0;
    return width / 2 - (chosen.offsetLeft + chosen.offsetWidth / 2);
  });
</script>

<div
  class="picker glass w-[min(100%,26rem)] overflow-clip rounded-full py-2"
  bind:clientWidth={width}
>
  <div
    bind:this={track}
    class="picker-track flex w-max gap-2"
    style:transform="translateX({shift}px)"
  >
    {#each options as option (option.id ?? 'standard')}
      <button
        type="button"
        class="shrink-0 rounded-full p-0.5 transition-transform duration-300 hover:scale-110"
        title={option.name}
        aria-label={option.name}
        aria-pressed={selected === option.id}
        onclick={() => onselect(option.id)}
      >
        <SkinIcon
          iconUrl={option.icon_url}
          iconColors={option.icon_colors}
          colors={option.colors}
          selected={selected === option.id}
          class="size-8"
        />
      </button>
    {/each}
  </div>
</div>

<style>
  /* The ends fade so liveries slide in and out of view. */
  .picker {
    mask-image: linear-gradient(90deg, transparent, black 18%, black 82%, transparent);
  }
  .picker-track {
    transition: transform 0.6s var(--ease-apple);
  }
</style>
