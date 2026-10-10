<script lang="ts">
  // What went wrong with the ship on stage, as a Photon alert: it could not
  // be built at all, or it was built with client files missing. Shown while
  // the stage glitches; nothing when the ship is fine.
  import PhotonAlert from '$lib/components/photon-alert.svelte';
  import { stage } from '$lib/stage.svelte';

  let { name, class: className = '' }: { name: string; class?: string } = $props();
</script>

{#if stage.status === 'error'}
  <PhotonAlert
    class={className}
    title="Ship unavailable"
    message="The {name} could not be assembled from the game client's files."
    details={stage.error ? [stage.error] : []}
    code="BUILD FAILED"
  />
{:else if stage.missing.length}
  <PhotonAlert
    class={className}
    title="Resources missing"
    message="{stage.missing.length === 1
      ? 'One of the client files'
      : `${stage.missing.length} client files`} for the {name} could not be loaded, so parts of it may be missing or drawn wrong."
    details={stage.missing}
    code="RES {stage.missing.length}"
  />
{/if}
