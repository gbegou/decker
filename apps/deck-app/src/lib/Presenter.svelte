<script lang="ts">
  // The presenter window of the editor: the shared presenter drawing
  // the current and next slides as SVG, with the deck's thumbnails.
  import { onMount } from "svelte";
  import { Presenter, SlideView, type ShowView } from "$shared";
  import { watchThumbnails } from "./api";

  let thumbnails = $state<string[]>([]);
  onMount(() => watchThumbnails((t) => (thumbnails = t)));
</script>

<Presenter thumbs={thumbnails}>
  {#snippet page(v: ShowView)}
    <SlideView svg={v.svg ?? ""} width={v.width} height={v.height} dir={v.dir} animated />
  {/snippet}
  {#snippet next(v: ShowView)}
    <SlideView svg={v.next_svg ?? ""} width={v.width} height={v.height} dir={v.dir} />
  {/snippet}
  {#snippet thumb(svg: string, _i: number, v: ShowView)}
    <SlideView {svg} width={v.width} height={v.height} />
  {/snippet}
</Presenter>
