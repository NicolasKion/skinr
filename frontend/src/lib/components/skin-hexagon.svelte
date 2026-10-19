<script lang="ts" module>
  /** The lights' color when a skin does not say: the client's default amber. */
  export const DEFAULT_LIGHTS = '#f2b234';
</script>

<script lang="ts">
  // The game's SKIN icon, the angled hexagon from the market and the SKINs
  // window, redrawn as vectors so it stays sharp at any size (the client's
  // own layers are 64 px).
  //
  // The hexagon is split into three sections that tile it exactly, following
  // the client's layers (res:/ui/texture/classes/skins/skin-icon-*.png) on
  // their 64-unit grid: the top band (secondary), the diagonal (hull) and the
  // bottom (primary: a left wing and the body). Neighbouring sections share
  // their boundary points, so they stitch together without gaps, and
  // everything a section shows (tint, patterns, shading, stripes, lights) is
  // clipped to it, so nothing spills into the next one.
  import type { IconPattern } from '#lib/skinr.js';
  import type { Colors } from '#lib/types.js';

  let {
    colors,
    patterns = [],
    lights = DEFAULT_LIGHTS,
    class: className = '',
  }: {
    colors: Colors;
    patterns?: IconPattern[];
    /** The color of the lit panels; skins tint them their own way. */
    lights?: string;
    class?: string;
  } = $props();

  /** The tint for a color the SDE leaves out. */
  const FALLBACK = '#5a5f66';

  // The hexagon's corners, as the client's layers cover it.
  const TOP_LEFT = '16 5';
  const TOP_RIGHT = '48 5';
  const RIGHT = '63 32';
  const BOTTOM_RIGHT = '48 59';
  const BOTTOM_LEFT = '16 59';
  const LEFT = '1 32';
  // Where the dividers meet the hexagon's sides (on the line between two corners).
  const TOP_EDGE_LEFT = '4.33 26';
  const TOP_EDGE_RIGHT = '55.22 18';
  const BODY_EDGE_RIGHT = '56.89 21';
  const BODY_EDGE_LEFT = '8.22 45';
  const WING_EDGE_LEFT = '7.11 43';

  // The dividers trace the client's `secondary` and `main` layers, which are
  // drawn over `hull`: the diagonal is what shows between them.
  /**
   * The top band's lower edge, left to right: over the wing (to the notch's
   * end), then over the diagonal, stepping up to its flat right end.
   */
  const TOP_DIVIDER = [
    TOP_EDGE_LEFT,
    '15 26',
    '18 30',
    '26 30',
    '31 25',
    '33 23',
    '35 21',
    '38 18',
    TOP_EDGE_RIGHT,
  ];
  /** Where the wing ends along the top divider. */
  const WING_END = 3;
  /** The wing's right edge, top to bottom. */
  const WING_DIVIDER = [TOP_DIVIDER[WING_END], '26 34', '17 43', WING_EDGE_LEFT];
  /** The body's upper edge, right to left. */
  const BODY_DIVIDER = [BODY_EDGE_RIGHT, '47 21', '38 30', '38 33.5', '27 45', BODY_EDGE_LEFT];

  const path = (points: string[]) => `M${points.join('L')}Z`;

  /** The three sections, each one or more closed paths. */
  const SECTIONS = {
    top: [path([TOP_LEFT, TOP_RIGHT, ...TOP_DIVIDER.toReversed()])],
    middle: [
      path([
        ...TOP_DIVIDER.slice(WING_END),
        ...BODY_DIVIDER,
        ...WING_DIVIDER.toReversed().slice(0, -1),
      ]),
    ],
    bottom: [
      path([...TOP_DIVIDER.slice(0, WING_END), ...WING_DIVIDER, LEFT]),
      path([
        BODY_EDGE_RIGHT,
        RIGHT,
        BOTTOM_RIGHT,
        BOTTOM_LEFT,
        ...BODY_DIVIDER.toReversed().slice(0, -1),
      ]),
    ],
  };
  type Section = keyof typeof SECTIONS;

  /**
   * Each section's grey down its height (y on the 64 grid, grey 0 to 1), as
   * measured from the client's layers: a bright one-unit edge on top, then a
   * fall-off. The icon is the tint times this grey, drawn as black over the
   * tint at 1 - grey.
   */
  const SHADING: Record<Section, [number, number][]> = {
    top: [
      [5, 0.98],
      [6.5, 0.72],
      [11, 0.67],
      [17, 0.56],
      [23, 0.47],
      [29, 0.31],
    ],
    middle: [
      [16, 0.39],
      [19, 0.46],
      [25, 0.4],
      [34, 0.44],
      [46, 0.42],
    ],
    bottom: [
      [21, 0.94],
      [22.5, 0.69],
      [27, 0.65],
      [33, 0.54],
      [39, 0.48],
      [42, 0.43],
      [48, 0.44],
      [54, 0.46],
      [56.5, 0.5],
      [58, 0.57],
    ],
  };
  /** The lights layer's grey: the panels show at this share of their color. */
  const LIGHTS_GREY = 0.75;
  /** The client's `lights` layer: small lit panels down the diagonal (x, y, w, h, brightness). */
  const LIGHTS = [
    [40, 20, 7, 2, 1],
    [37, 22, 8, 2, 0.8],
    [38, 24, 1, 1, 0.5],
    [32, 24, 2, 2, 0.7],
    [26, 32, 3, 2, 0.6],
    [31, 32, 2, 2, 0.9],
    [33, 34, 3, 2, 0.75],
    [29, 36, 6, 1, 0.55],
    [33, 37, 1, 1, 0.4],
    [19, 41, 2, 1, 0.5],
    [25, 41, 2, 1, 0.65],
    [28, 41, 2, 1, 0.45],
    [23, 42, 4, 1, 0.8],
    [21, 43, 7, 1, 0.9],
    [22, 44, 5, 1, 0.6],
  ];
  /**
   * The lights are soft in the client, smudges rather than panels: each is a
   * blurred glow with a slightly blurred core, the last value above its brightness.
   */
  const LIGHT_BLUR = { glow: 0.7, core: 0.25, glowOpacity: 0.55 };

  /**
   * The recessed slot near the body's bottom edge (the client's `main` layer,
   * rows 53 to 58): a shadowed outline and a lit left bevel.
   */
  const SLOT = {
    shadow: { d: 'M23 57.5L26.5 53.5H40.5L44 57.5', opacity: 0.4 },
    bevel: { d: 'M27 55.2L24.6 57.5', opacity: 0.35 },
    inner: { d: 'M27.5 55.2H40', opacity: 0.1 },
  };
  /** The body's lit rim along the diagonal (the client's `main` layer is brighter there). */
  const RIM = { opacity: 0.18, width: 1.6 };
  /** The dark seam between sections, like the gaps between the client's layers. */
  /** Blurred a little, so edges fade like the client's layer edges instead of cutting. */
  const SEAM = { color: '#08090b', width: 0.9, opacity: 0.8, blur: 0.35 };

  const uid = $props.id();
  const id = (name: string) => `skin-hexagon-${uid}-${name}`;

  const sections = $derived(
    (
      [
        ['top', colors.secondary ?? colors.primary],
        ['middle', colors.hull],
        ['bottom', colors.primary],
      ] as const
    ).map(([name, tint]) => ({ name, paths: SECTIONS[name], tint: tint ?? FALLBACK })),
  );
</script>

<svg viewBox="0 0 64 64" class="skin-hexagon block {className}" aria-hidden="true">
  <defs>
    {#each sections as section (section.name)}
      <clipPath id={id(`clip-${section.name}`)}>
        {#each section.paths as d (d)}<path {d} />{/each}
      </clipPath>
      <linearGradient
        id={id(`shade-${section.name}`)}
        gradientUnits="userSpaceOnUse"
        x1="0"
        y1="0"
        x2="0"
        y2="64"
      >
        {#each SHADING[section.name] as [y, grey] (y)}
          <stop offset={y / 64} stop-color="#000" stop-opacity={1 - grey} />
        {/each}
      </linearGradient>
    {/each}
    <pattern id={id('stripes')} width="64" height="1.6" patternUnits="userSpaceOnUse">
      <rect width="64" height="0.5" fill="#000" fill-opacity="0.18" />
    </pattern>
    {#each patterns as pattern, index (index)}
      <mask
        id={id(`pattern-${index}`)}
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="64"
        height="64"
      >
        <image href={pattern.url} x="0" y="0" width="64" height="64" preserveAspectRatio="none" />
      </mask>
    {/each}
    <filter
      color-interpolation-filters="sRGB"
      id={id('light-glow')}
      filterUnits="userSpaceOnUse"
      x="0"
      y="0"
      width="64"
      height="64"
    >
      <feGaussianBlur stdDeviation={LIGHT_BLUR.glow} />
    </filter>
    <filter
      color-interpolation-filters="sRGB"
      id={id('light-core')}
      filterUnits="userSpaceOnUse"
      x="0"
      y="0"
      width="64"
      height="64"
    >
      <feGaussianBlur stdDeviation={LIGHT_BLUR.core} />
    </filter>
    <filter
      color-interpolation-filters="sRGB"
      id={id('seam')}
      filterUnits="userSpaceOnUse"
      x="-4"
      y="-4"
      width="72"
      height="72"
    >
      <feGaussianBlur stdDeviation={SEAM.blur} />
    </filter>
  </defs>

  <!-- Each section draws only inside itself. -->
  {#each sections as section (section.name)}
    <g clip-path="url(#{id(`clip-${section.name}`)})">
      <rect width="64" height="64" fill={section.tint} />
      {#each patterns as pattern, index (index)}
        <rect width="64" height="64" fill={pattern.color} mask="url(#{id(`pattern-${index}`)})" />
      {/each}
      <rect width="64" height="64" fill="url(#{id(`shade-${section.name}`)})" />
      {#if section.name === 'middle'}
        <rect width="64" height="64" fill="url(#{id('stripes')})" />
        <g fill={lights}>
          <g filter="url(#{id('light-glow')})" opacity={LIGHT_BLUR.glowOpacity}>
            {#each LIGHTS as [x, y, width, height, brightness] (`${x},${y}`)}
              <rect {x} {y} {width} {height} fill-opacity={brightness} />
            {/each}
          </g>
          <g filter="url(#{id('light-core')})" opacity={LIGHTS_GREY}>
            {#each LIGHTS as [x, y, width, height, brightness] (`${x},${y}`)}
              <rect
                x={x + 0.25}
                y={y + 0.2}
                width={width - 0.5}
                height={height - 0.4}
                fill-opacity={brightness}
              />
            {/each}
          </g>
        </g>
      {:else if section.name === 'bottom'}
        <path
          d="M{BODY_DIVIDER.join('L')}"
          fill="none"
          stroke="#fff"
          stroke-opacity={RIM.opacity}
          stroke-width={RIM.width}
        />
        <g fill="none" stroke-width="0.8" stroke-linejoin="miter">
          <path d={SLOT.shadow.d} stroke="#000" stroke-opacity={SLOT.shadow.opacity} />
          <path d={SLOT.bevel.d} stroke="#fff" stroke-opacity={SLOT.bevel.opacity} />
          <path d={SLOT.inner.d} stroke="#fff" stroke-opacity={SLOT.inner.opacity} />
        </g>
      {/if}
    </g>
  {/each}

  <!-- The seams where the sections meet, and the outline. -->
  <g
    fill="none"
    stroke={SEAM.color}
    stroke-width={SEAM.width}
    stroke-opacity={SEAM.opacity}
    filter="url(#{id('seam')})"
  >
    {#each sections as section (section.name)}
      {#each section.paths as d (d)}<path {d} />{/each}
    {/each}
  </g>
</svg>
