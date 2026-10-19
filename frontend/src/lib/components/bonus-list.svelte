<script lang="ts">
  // A ship's bonuses as the game lists them: per skill level, then role.
  import EveText from '#lib/components/eve-text.svelte';
  import type { BonusGroup } from '#lib/types.js';

  let { groups }: { groups: BonusGroup[] } = $props();
</script>

<div class="space-y-6">
  {#each groups as group (group.title)}
    <section>
      <h4 class="mb-3 flex items-baseline gap-3">
        <span class="font-mono text-[0.72rem] tracking-[0.1em] text-white uppercase">
          {group.title}
        </span>
        {#if group.per_level}
          <span
            class="ml-auto font-mono text-[0.62rem] tracking-[0.12em] text-[#8fc8ff]/55 uppercase"
          >
            per level
          </span>
        {/if}
      </h4>
      <ul class="space-y-2">
        {#each group.lines as line, index (index)}
          <li class="flex items-start gap-3 text-[0.9rem] leading-snug">
            <span
              class="w-12 shrink-0 text-right font-mono text-[0.85rem] font-medium tabular-nums"
              class:amount={line.amount}
              class:bullet={!line.amount}
            >
              {line.amount ?? '·'}
            </span>
            <span class="text-foreground/80"><EveText segments={line.text} /></span>
          </li>
        {/each}
      </ul>
    </section>
  {:else}
    <p class="font-mono text-sm text-[#8fc8ff]/60">This hull carries no bonuses.</p>
  {/each}
</div>

<style>
  .amount {
    color: #6cc4ff;
  }
  .bullet {
    color: rgb(108 196 255 / 0.4);
  }
</style>
