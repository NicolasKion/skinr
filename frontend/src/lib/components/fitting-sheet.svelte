<script lang="ts">
  // The fitting window's left half: slot racks as drafted bays, hardpoints,
  // and the resources a fit spends.
  import AttributeIcon from '$lib/components/attribute-icon.svelte';
  import SpecRow from '$lib/components/spec-row.svelte';
  import type { Sheet } from '$lib/types';

  let { sheet }: { sheet: Sheet } = $props();

  /** The most slots any rack has; every rack draws this many bays so they line up. */
  const BAYS = 8;

  const number = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });

  const racks = $derived([
    { label: 'High', icon: 'hiSlots', count: sheet.slots.high },
    { label: 'Mid', icon: 'medSlots', count: sheet.slots.mid },
    { label: 'Low', icon: 'lowSlots', count: sheet.slots.low },
    { label: 'Rig', icon: 'upgradeSlotsLeft', count: sheet.slots.rig },
  ]);

  const hardpoints = $derived([
    { label: 'Turret', icon: 'turretSlotsLeft', count: sheet.slots.turret },
    { label: 'Launcher', icon: 'launcherSlotsLeft', count: sheet.slots.launcher },
  ]);

  const resources = $derived(
    [
      { label: 'Powergrid', icon: 'powerOutput', value: sheet.fitting.powergrid, unit: 'MW' },
      { label: 'CPU', icon: 'cpuOutput', value: sheet.fitting.cpu, unit: 'tf' },
      { label: 'Calibration', icon: 'upgradeCapacity', value: sheet.fitting.calibration, unit: '' },
      { label: 'Drone bay', icon: 'droneCapacity', value: sheet.fitting.drone_bay, unit: 'm³' },
      {
        label: 'Bandwidth',
        icon: 'droneBandwidth',
        value: sheet.fitting.drone_bandwidth,
        unit: 'Mbit/s',
      },
      {
        label: 'Fighter bay',
        icon: 'fighterCapacity',
        value: sheet.fitting.fighter_bay,
        unit: 'm³',
      },
      {
        label: 'Fighter tubes',
        icon: 'fighterTubes',
        value: sheet.fitting.fighter_tubes,
        unit: '',
      },
    ].filter((resource) => resource.value !== null),
  );
</script>

<div class="space-y-2">
  {#each racks as rack (rack.label)}
    <div class="flex items-center gap-3">
      <AttributeIcon name={rack.icon} size={20} class="opacity-80" />
      <span class="w-10 font-mono text-[0.72rem] tracking-[0.08em] text-[#8fc8ff]/75 uppercase">
        {rack.label}
      </span>
      <div class="flex flex-1 gap-1">
        {#each { length: Math.max(BAYS, rack.count) }, index (index)}
          <span
            class="bay h-6 flex-1"
            class:fitted={index < rack.count}
            style:animation-delay="{index * 50}ms"
          ></span>
        {/each}
      </div>
      <span class="w-6 text-right font-mono text-base font-medium text-white tabular-nums">
        {rack.count}
      </span>
    </div>
  {/each}
</div>

<div class="mt-6 flex gap-10">
  {#each hardpoints as hardpoint (hardpoint.label)}
    <div class="flex items-center gap-3">
      <AttributeIcon name={hardpoint.icon} size={32} />
      <div>
        <p class="font-mono text-xl leading-none font-medium text-white tabular-nums">
          {hardpoint.count}
        </p>
        <p class="mt-1 font-mono text-[0.65rem] tracking-[0.12em] text-[#8fc8ff]/60 uppercase">
          {hardpoint.label} hardpoints
        </p>
      </div>
    </div>
  {/each}
</div>

<div class="mt-7">
  {#each resources as resource (resource.label)}
    <SpecRow
      icon={resource.icon}
      label={resource.label}
      value={number.format(resource.value ?? 0)}
      unit={resource.unit}
    />
  {/each}
</div>

<style>
  /* An empty bay is drafted as a dashed outline; a fitted one is filled. */
  .bay {
    border: 1px dashed rgb(108 196 255 / 0.18);
    animation: bay-in 0.6s var(--ease-out-expo) both;
  }
  .bay.fitted {
    border: 1px solid rgb(108 196 255 / 0.65);
    background: rgb(108 196 255 / 0.16);
    box-shadow: 0 0 10px -3px rgb(108 196 255 / 0.6);
  }
  @keyframes bay-in {
    from {
      opacity: 0;
      transform: scaleY(0.4);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bay {
      animation: none;
    }
  }
</style>
