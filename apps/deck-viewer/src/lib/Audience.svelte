<script lang="ts">
  import { Audience, type ShowView } from "$shared";
  import PdfPage from "./PdfPage.svelte";
  import type { View } from "./api";
</script>

<Audience>
  {#snippet page(v: ShowView)}
    {#if (v as View).pdf}<PdfPage path={(v as View).pdf!} index={v.slide} width={v.width} height={v.height} />{/if}
  {/snippet}
</Audience>
