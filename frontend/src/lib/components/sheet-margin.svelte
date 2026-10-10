<script lang="ts">
  // The margin band of an engineering drawing, along the top or bottom edge
  // of the blueprint paper: a ruler of fine ticks, the sheet split into
  // numbered zones, and a centring mark pointing into the sheet. It frames the
  // paper so the step from the dark chapters reads as the edge of a drawing.
  let { edge }: { edge: 'top' | 'bottom' } = $props();

  /** How many zones the sheet is divided into, like a drawing's grid references. */
  const ZONES = 8;
</script>

<div class="margin {edge}" aria-hidden="true">
  <div class="ruler"></div>
  <div class="zones" style:grid-template-columns="repeat({ZONES}, 1fr)">
    {#each { length: ZONES }, index (index)}
      <span>{index + 1}</span>
    {/each}
  </div>
  <span class="centre"></span>
</div>

<style>
  .margin {
    position: absolute;
    inset-inline: 0;
    height: 2.5rem;
    color: rgb(143 200 255 / 0.55);
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.1em;
  }
  .margin.top {
    top: 0;
    border-bottom: 1px solid var(--blueprint-line);
  }
  .margin.bottom {
    bottom: 0;
    border-top: 1px solid var(--blueprint-line);
  }
  /* Fine ticks every 12px, longer ones every 96px, standing on the frame line. */
  .ruler {
    position: absolute;
    inset-inline: 0;
    height: 0.75rem;
    background:
      repeating-linear-gradient(90deg, var(--blueprint-line) 0 1px, transparent 1px 96px) 0 0 / 100%
        100% no-repeat,
      repeating-linear-gradient(90deg, rgb(77 134 184 / 0.6) 0 1px, transparent 1px 12px) 0 100% /
        100% 45% no-repeat;
  }
  .top .ruler {
    bottom: 0;
  }
  .bottom .ruler {
    top: 0;
    transform: scaleY(-1);
  }
  /* The zones: numbered cells divided by the frame's own lines. */
  .zones {
    position: absolute;
    inset-inline: 0;
    display: grid;
    height: 1.4rem;
  }
  .top .zones {
    top: 0.2rem;
  }
  .bottom .zones {
    bottom: 0.2rem;
  }
  .zones span {
    display: grid;
    place-items: center;
    border-left: 1px solid rgb(77 134 184 / 0.5);
  }
  .zones span:first-child {
    border-left: 0;
  }
  /* The centring mark: a small triangle at mid-width pointing into the sheet. */
  .centre {
    position: absolute;
    left: 50%;
    width: 0;
    height: 0;
    border-inline: 6px solid transparent;
    transform: translateX(-50%);
  }
  .top .centre {
    bottom: -7px;
    border-top: 7px solid var(--blueprint-ink);
  }
  .bottom .centre {
    top: -7px;
    border-bottom: 7px solid var(--blueprint-ink);
  }
</style>
