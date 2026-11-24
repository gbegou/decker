<script lang="ts">
  // The projector window: the current slide, letterboxed on black.
  import { onMount } from "svelte";
  import { api, watchView, presentationKeys, type View } from "./api";
  import SlideView from "./SlideView.svelte";

  let view = $state<View | null>(null);
  onMount(() => watchView((v) => (view = v)));

  function onKey(e: KeyboardEvent) {
    if (presentationKeys(e)) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="audience" style="background: {view?.blank === 'white' ? 'white' : 'black'}" onclick={() => api.navigate("next")} oncontextmenu={(e) => { e.preventDefault(); api.navigate("prev"); }}>
  {#if view && !view.blank}
    <div class="frame" style="aspect-ratio: {view.width} / {view.height}">
      <SlideView svg={view.svg} width={view.width} height={view.height} dir={view.dir} mode="show" animated />
    </div>
  {/if}
</div>

<style>
  .audience { width: 100vw; height: 100vh; background: black; display: flex; align-items: center; justify-content: center; cursor: none; }
  .frame { max-width: 100vw; max-height: 100vh; width: 100vw; }
  @media (min-aspect-ratio: 16/9) { .frame { width: auto; height: 100vh; } }
</style>
