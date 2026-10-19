<script lang="ts">
  // EVE HTML as the API hands it over: styled segments, rendered as text
  // nodes and plain elements only (never {@html}), with links to ship pages.
  // `reveal` wraps every word so it lights up as the reader scrolls past.
  import type { RichSegment } from '#lib/types.js';

  let { segments, reveal = false }: { segments: RichSegment[]; reveal?: boolean } = $props();

  /** Words and the whitespace between them, so wrapping each word keeps the spacing. */
  const pieces = (text: string) => text.split(/(\s+)/).filter((piece) => piece.length > 0);
</script>

{#snippet words(text: string)}
  {#if reveal}
    {#each pieces(text) as piece, index (index)}
      {#if /^\s+$/.test(piece)}{piece}{:else}<span class="reveal-word">{piece}</span>{/if}
    {/each}
  {:else}
    {text}
  {/if}
{/snippet}

{#each segments as segment, index (index)}
  {#if 'br' in segment}
    <br />
  {:else if segment.type_id}
    <a
      href="/ships/{segment.type_id}"
      class="underline decoration-foreground/25 underline-offset-4 hover:decoration-current"
      class:font-bold={segment.bold}
      class:italic={segment.italic}
      style:color={segment.color}>{@render words(segment.text)}</a
    >
  {:else}
    <span
      class:font-bold={segment.bold}
      class:italic={segment.italic}
      class:underline={segment.underline}
      style:color={segment.color}>{@render words(segment.text)}</span
    >
  {/if}
{/each}
