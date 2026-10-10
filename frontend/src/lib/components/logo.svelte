<script lang="ts">
  // The skinr.space mark: EVE's SKIN icon, an angled hexagon split into two
  // bands by a stepped diagonal, drawn in the current color (the lower band
  // dimmed).
  let { class: className = '' }: { class?: string } = $props();

  /** The hexagon, used as both outline and clip. */
  const HEXAGON = 'M9 4.5h14L29.5 16 23 27.5H9L2.5 16Z';
  /** How far the lower band is dimmed. */
  const LOWER_BAND_OPACITY = 0.42;

  // Each mark on a page needs its own clip path id.
  const uid = $props.id();
  const clip = `skinr-mark-${uid}`;
</script>

<svg viewBox="0 0 32 32" class={className} aria-hidden="true">
  <defs>
    <clipPath id={clip}><path d={HEXAGON} /></clipPath>
  </defs>
  <g clip-path="url(#{clip})" fill="currentColor">
    <path d="M0 0H32V11H21.5L12.5 20H0Z" />
    <path d="M0 22H13.5L22.5 13H32V32H0Z" fill-opacity={LOWER_BAND_OPACITY} />
  </g>
  <path d={HEXAGON} fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
</svg>
