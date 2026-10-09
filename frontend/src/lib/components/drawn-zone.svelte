<script lang="ts">
  // One zone of the blueprint sheet: its frame draws itself in, line by line
  // (from the top left corner, clockwise) once `built`, after `delay`; the
  // content follows as the line closes. Zones sit edge to edge, so together
  // they read as one sheet divided by its lines, not as separate cards.
  import type { Snippet } from 'svelte';

  let {
    built,
    delay = 0,
    class: className = '',
    children,
  }: { built: boolean; delay?: number; class?: string; children: Snippet } = $props();

  /** How long the frame takes to draw itself. */
  const DRAW_MS = 1100;
</script>

<div
  class="zone relative {className}"
  class:built
  style:--delay="{delay}ms"
  style:--draw="{DRAW_MS}ms"
>
  <!-- Top, right, bottom, left: drawn one after another, clockwise. -->
  <span class="line top" aria-hidden="true"></span>
  <span class="line right" aria-hidden="true"></span>
  <span class="line bottom" aria-hidden="true"></span>
  <span class="line left" aria-hidden="true"></span>
  <div class="content">
    {@render children()}
  </div>
</div>

<style>
  .line {
    position: absolute;
    pointer-events: none;
    background: var(--blueprint-line);
    transition: transform calc(var(--draw) / 4) linear;
  }
  .top,
  .bottom {
    left: 0;
    right: 0;
    height: 1px;
    transform: scaleX(0);
  }
  .left,
  .right {
    top: 0;
    bottom: 0;
    width: 1px;
    transform: scaleY(0);
  }
  .top {
    top: 0;
    transform-origin: left;
    transition-delay: var(--delay);
  }
  .right {
    right: 0;
    transform-origin: top;
    transition-delay: calc(var(--delay) + var(--draw) / 4);
  }
  .bottom {
    bottom: 0;
    transform-origin: right;
    transition-delay: calc(var(--delay) + var(--draw) / 2);
  }
  .left {
    left: 0;
    transform-origin: bottom;
    transition-delay: calc(var(--delay) + var(--draw) * 3 / 4);
  }
  .built .line {
    transform: none;
  }
  .content {
    position: relative;
    /* Opacity only: the drawing zone holds the stage's dock, which must not move. */
    opacity: 0;
    transition: opacity 0.7s var(--ease-out-expo) calc(var(--delay) + var(--draw) * 0.5);
  }
  .built .content {
    opacity: 1;
  }
  @media (prefers-reduced-motion: reduce) {
    .line,
    .content {
      transition: none;
    }
  }
</style>
