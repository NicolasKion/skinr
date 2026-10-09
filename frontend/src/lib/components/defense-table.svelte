<script lang="ts">
  // The three defense layers as a table: hit points, then one column per
  // damage type with the game's resist icon and color.
  import AttributeIcon from '$lib/components/attribute-icon.svelte';
  import type { Sheet } from '$lib/types';

  let { sheet }: { sheet: Sheet } = $props();

  /** The colors and icons the game uses for each damage type. */
  const DAMAGE = [
    { key: 'em', label: 'EM', icon: 'emDamageResonance', color: '#4f8fd6' },
    { key: 'thermal', label: 'Therm', icon: 'thermalDamageResonance', color: '#d6534f' },
    { key: 'kinetic', label: 'Kin', icon: 'kineticDamageResonance', color: '#a3a9b0' },
    { key: 'explosive', label: 'Exp', icon: 'explosiveDamageResonance', color: '#e0a03a' },
  ] as const;

  /** Each layer's hit point attribute, for its icon. */
  const LAYER_ICONS: Record<string, string> = {
    Shield: 'shieldCapacity',
    Armor: 'armorHP',
    Structure: 'hp',
  };

  const whole = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });
  const total = $derived(sheet.defense.reduce((sum, layer) => sum + layer.hp, 0));
</script>

<table class="w-full border-collapse font-mono tabular-nums">
  <thead>
    <tr>
      <th class="pb-2 text-left">Layer</th>
      <th class="pb-2 text-right">HP</th>
      {#each DAMAGE as damage (damage.key)}
        <th class="w-[17%] pb-2">
          <span class="flex items-center justify-end gap-1">
            <AttributeIcon name={damage.icon} size={20} />
            {damage.label}
          </span>
        </th>
      {/each}
    </tr>
  </thead>
  <tbody>
    {#each sheet.defense as layer (layer.name)}
      <tr>
        <td class="py-3">
          <span class="flex items-center gap-2.5">
            <AttributeIcon name={LAYER_ICONS[layer.name] ?? 'hp'} size={28} />
            <span>
              <span class="block text-sm font-medium text-white">{layer.name}</span>
              {#if layer.recharge}
                <span class="block text-[0.68rem] text-[#8fc8ff]/55">
                  {whole.format(layer.recharge)} s recharge
                </span>
              {/if}
            </span>
          </span>
        </td>
        <td class="py-3 text-right text-[0.95rem] font-medium text-white">
          {whole.format(layer.hp)}
        </td>
        {#each DAMAGE as damage (damage.key)}
          {@const resist = layer[damage.key]}
          <td class="py-3 pl-3 text-right">
            <span class="text-sm text-white/90">{whole.format(resist)}%</span>
            <span class="mt-1.5 block h-1 bg-white/[0.06]">
              <span
                class="bar block h-full"
                style:width="{Math.max(0, Math.min(100, resist))}%"
                style:background={damage.color}
              ></span>
            </span>
          </td>
        {/each}
      </tr>
    {/each}
  </tbody>
  <tfoot>
    <tr>
      <td class="pt-3 text-[0.68rem] tracking-[0.12em] text-[#8fc8ff]/60 uppercase">Total</td>
      <td class="pt-3 text-right text-[0.95rem] font-semibold text-[#6cc4ff]">
        {whole.format(total)}
      </td>
      <td colspan={DAMAGE.length}></td>
    </tr>
  </tfoot>
</table>

<style>
  th {
    font-size: 0.65rem;
    font-weight: 500;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    text-align: right;
    color: rgb(143 200 255 / 0.65);
  }
  th:first-child {
    text-align: left;
  }
  .bar {
    transform-origin: left;
    animation: bar-in 1.1s var(--ease-out-expo) both;
  }
  @keyframes bar-in {
    from {
      transform: scaleX(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar {
      animation: none;
    }
  }
</style>
