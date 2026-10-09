<script lang="ts">
  // One line of a spec table: the attribute's game icon, its label, a dotted
  // leader, then the unit and the figure in columns of their own, so figures
  // in a list line up on their last digit.
  import AttributeIcon from '$lib/components/attribute-icon.svelte';

  let {
    icon,
    label,
    value,
    unit = '',
  }: { icon: string; label: string; value: string; unit?: string } = $props();
</script>

<div class="row flex items-center gap-3 py-1.5">
  <AttributeIcon name={icon} size={24} class="opacity-90" />
  <span class="label">{label}</span>
  <span class="leader" aria-hidden="true"></span>
  <span class="unit">{unit}</span>
  <span
    class="min-w-[4.5rem] text-right font-mono text-[0.95rem] font-medium text-white tabular-nums"
  >
    {value}
  </span>
</div>

<style>
  .label {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgb(143 200 255 / 0.75);
    white-space: nowrap;
  }
  .unit {
    width: 3.25rem;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: rgb(143 200 255 / 0.55);
  }
  .leader {
    flex: 1;
    min-width: 1rem;
    align-self: flex-end;
    margin-bottom: 0.45rem;
    border-bottom: 1px dotted rgb(108 196 255 / 0.3);
  }
</style>
