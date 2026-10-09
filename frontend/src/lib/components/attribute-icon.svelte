<script lang="ts">
  // The game client's own icon for a dogma attribute (served by the API from
  // the client resources the SDE import fetched). Keeps its box when missing
  // so rows stay aligned.
  let {
    name,
    size = 20,
    class: className = '',
  }: { name: string; size?: number; class?: string } = $props();

  let failed = $state(false);
</script>

{#if failed}
  <span class="inline-block shrink-0 {className}" style:width="{size}px" style:height="{size}px"
  ></span>
{:else}
  <img
    class="inline-block shrink-0 {className}"
    src="/api/icons/{name}.png"
    alt=""
    width={size}
    height={size}
    loading="lazy"
    onerror={() => (failed = true)}
  />
{/if}
