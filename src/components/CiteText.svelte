<script lang="ts">
  import { parseCitations } from "../lib/mock";

  let { text }: { text: string } = $props();

  const segments = $derived(parseCitations(text));
</script>

{#each segments as seg}
  {#if seg.t === "cite"}
    <span class="cite">{seg.v}</span>
  {:else}
    {seg.v}
  {/if}
{/each}
