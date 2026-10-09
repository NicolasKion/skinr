<script lang="ts">
  // Capacitor, targeting and navigation as parts-list rows, each with the
  // game's icon for the attribute behind it.
  import SpecRow from '$lib/components/spec-row.svelte';
  import type { Sheet } from '$lib/types';

  let { sheet }: { sheet: Sheet } = $props();

  const number = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
  const whole = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });
  const fine = new Intl.NumberFormat('en-US', { maximumFractionDigits: 3 });

  type Row = {
    icon: string;
    label: string;
    amount: number | null;
    unit: string;
    format?: Intl.NumberFormat;
  };

  const groups = $derived(
    (
      [
        {
          title: 'Capacitor',
          rows: [
            {
              icon: 'capacitorCapacity',
              label: 'Capacity',
              amount: sheet.capacitor.capacity,
              unit: 'GJ',
            },
            {
              icon: 'rechargeRate',
              label: 'Recharge',
              amount: sheet.capacitor.recharge,
              unit: 's',
            },
          ],
        },
        {
          title: 'Targeting',
          rows: [
            { icon: 'maxTargetRange', label: 'Range', amount: sheet.targeting.range, unit: 'km' },
            {
              icon: 'maxLockedTargets',
              label: 'Locked targets',
              amount: sheet.targeting.max_targets,
              unit: '',
            },
            {
              icon: 'scanResolution',
              label: 'Scan resolution',
              amount: sheet.targeting.scan_resolution,
              unit: 'mm',
            },
            {
              icon: 'signatureRadius',
              label: 'Signature',
              amount: sheet.targeting.signature_radius,
              unit: 'm',
            },
            {
              icon: sheet.targeting.sensor
                ? `scan${sheet.targeting.sensor.kind}Strength`
                : 'scanRadarStrength',
              label: sheet.targeting.sensor ? `${sheet.targeting.sensor.kind}` : 'Sensor',
              amount: sheet.targeting.sensor?.strength ?? null,
              unit: 'pts',
            },
          ],
        },
        {
          title: 'Navigation',
          rows: [
            {
              icon: 'maxVelocity',
              label: 'Velocity',
              amount: sheet.navigation.max_velocity,
              unit: 'm/s',
              format: whole,
            },
            {
              icon: 'agility',
              label: 'Align time',
              amount: sheet.navigation.align_time,
              unit: 's',
            },
            {
              icon: 'agility',
              label: 'Inertia',
              amount: sheet.navigation.inertia,
              unit: 'x',
              format: fine,
            },
            {
              icon: 'baseWarpSpeed',
              label: 'Warp speed',
              amount: sheet.navigation.warp_speed,
              unit: 'AU/s',
            },
          ],
        },
      ] as { title: string; rows: Row[] }[]
    )
      .map((group) =>
        Object.assign(group, { rows: group.rows.filter((row) => row.amount !== null) }),
      )
      .filter((group) => group.rows.length),
  );
</script>

<div class="space-y-6">
  {#each groups as group (group.title)}
    <div>
      <p class="mb-1 font-mono text-[0.65rem] tracking-[0.18em] text-[#6cc4ff] uppercase">
        {group.title}
      </p>
      {#each group.rows as row (row.label)}
        <SpecRow
          icon={row.icon}
          label={row.label}
          value={(row.format ?? number).format(row.amount ?? 0)}
          unit={row.unit}
        />
      {/each}
    </div>
  {/each}
</div>
