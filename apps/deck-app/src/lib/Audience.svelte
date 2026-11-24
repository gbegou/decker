<script lang="ts">
  // The projector window of the editor: the shared audience window
  // drawing the slide's SVG, with media playing.
  import { Audience, SlideView, type ShowView } from "$shared";
</script>

<Audience>
  {#snippet page(v: ShowView)}
    <SlideView svg={v.svg ?? ""} width={v.width} height={v.height} dir={v.dir} mode="show" animated />
  {/snippet}
</Audience>
