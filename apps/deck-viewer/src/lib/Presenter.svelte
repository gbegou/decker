<script lang="ts">
  // The presenter window for a PDF: pages drawn by pdf.js, one
  // thumbnail per page.
  import { Presenter, type ShowView } from "$shared";
  import PdfPage from "./PdfPage.svelte";
  import type { View } from "./api";
  import { onMount } from "svelte";
  import { watchView } from "./api";

  let pages = $state<number[]>([]);
  onMount(() => watchView((v) => { if (pages.length !== v.slide_count) pages = Array.from({ length: v.slide_count }, (_, i) => i); }));
  const pdf = (v: ShowView) => (v as View).pdf;
</script>

<Presenter thumbs={pages}>
  {#snippet page(v: ShowView)}
    {#if pdf(v)}<PdfPage path={pdf(v)!} index={v.slide} width={v.width} height={v.height} fit="box" />{/if}
  {/snippet}
  {#snippet next(v: ShowView)}
    {#if pdf(v)}<PdfPage path={pdf(v)!} index={v.slide + 1} width={v.width} height={v.height} />{/if}
  {/snippet}
  {#snippet thumb(i: number, _k: number, v: ShowView)}
    {#if pdf(v)}<PdfPage path={pdf(v)!} index={i} width={v.width} height={v.height} />{/if}
  {/snippet}
</Presenter>
