<script lang="ts">
  // An alert in the style of EVE's Photon UI: a dark glass plate with one
  // chamfered corner, a warning stripe down its edge, a tracked uppercase
  // caption beside the warning triangle, and the details in small type. It
  // flickers in like a signal coming back.

  let {
    title,
    message,
    details = [],
    code = null,
    class: className = '',
  }: {
    title: string;
    message: string;
    /** Lines listed under the message (resource paths); the first few are shown. */
    details?: string[];
    /** A short reference shown in the corner, like the client's error codes. */
    code?: string | null;
    class?: string;
  } = $props();

  /** How many detail lines are listed before the rest are counted. */
  const SHOWN_DETAILS = 3;

  const shown = $derived(details.slice(0, SHOWN_DETAILS));
  const more = $derived(details.length - shown.length);
</script>

<div class="photon-alert {className}" role="alert">
  <div class="flex items-center gap-2.5">
    <svg class="size-4 shrink-0" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M8 1.5 15 14.5H1Z" fill="none" stroke="currentColor" stroke-width="1.4" />
      <path d="M8 6v4.2" stroke="currentColor" stroke-width="1.6" />
      <rect x="7.2" y="11.4" width="1.6" height="1.6" fill="currentColor" />
    </svg>
    <p class="caption">{title}</p>
    {#if code}
      <p class="code ml-auto">{code}</p>
    {/if}
  </div>
  <p class="mt-2.5 text-[0.8rem] leading-relaxed text-white/75">{message}</p>
  {#if shown.length}
    <ul class="mt-3 space-y-1">
      {#each shown as line (line)}
        <li class="detail truncate" title={line}>{line}</li>
      {/each}
      {#if more > 0}
        <li class="detail text-white/40">and {more} more</li>
      {/if}
    </ul>
  {/if}
</div>

<style>
  .photon-alert {
    --warning: #f5a524;
    position: relative;
    max-width: 26rem;
    padding: 0.9rem 1.1rem 1rem 1.35rem;
    color: var(--warning);
    background: linear-gradient(90deg, rgb(245 165 36 / 0.1), transparent 40%), rgb(8 10 12 / 0.82);
    backdrop-filter: blur(14px) saturate(1.2);
    /* Photon's chamfered corner, top left. */
    clip-path: polygon(14px 0, 100% 0, 100% 100%, 0 100%, 0 14px);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1);
    animation: signal-in 0.6s steps(1, end) both;
  }
  /* The warning stripe down the left edge. */
  .photon-alert::before {
    content: '';
    position: absolute;
    inset: 14px auto 0 0;
    width: 3px;
    background: var(--warning);
  }
  /* The chamfer's own edge line. */
  .photon-alert::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    width: 20px;
    height: 1px;
    background: var(--warning);
    transform-origin: left;
    transform: translate(0, 13.5px) rotate(-45deg);
  }
  .caption {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }
  .code {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.1em;
    color: rgb(255 255 255 / 0.35);
  }
  .detail {
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: rgb(255 255 255 / 0.55);
  }
  /* Flickers in like a signal finding its lock. */
  @keyframes signal-in {
    0% {
      opacity: 0;
    }
    15% {
      opacity: 1;
      transform: translateX(-6px);
    }
    25% {
      opacity: 0.2;
    }
    40% {
      opacity: 1;
      transform: translateX(3px);
    }
    55% {
      opacity: 0.5;
    }
    70%,
    100% {
      opacity: 1;
      transform: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .photon-alert {
      animation: none;
    }
  }
</style>
