<script lang="ts">
  // The projector window: the current slide, letterboxed on black.
  import { onMount } from "svelte";
  import { api, watchView, presentationKeys, type View } from "./api";
  import SlideView from "./SlideView.svelte";
  import { watchPointer, type Pointer } from "./pointer";

  let view = $state<View | null>(null);
  let pointer = $state<Pointer | null>(null);
  onMount(() => {
    const stopView = watchView((v) => (view = v));
    const stopPointer = watchPointer((p) => (pointer = p));
    return () => { stopView(); stopPointer(); };
  });

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
      {#if pointer}<div class="dot" style="left: {pointer.x * 100}%; top: {pointer.y * 100}%"></div>{/if}
    </div>
  {/if}
</div>

<style>
  .audience { width: 100vw; height: 100vh; background: black; display: flex; align-items: center; justify-content: center; cursor: none; }
  .frame { position: relative; max-width: 100vw; max-height: 100vh; width: 100vw; }
  .dot { position: absolute; width: 1.6vmin; height: 1.6vmin; margin: -0.8vmin 0 0 -0.8vmin; border-radius: 50%; background: #e53935; box-shadow: 0 0 1vmin 0.4vmin rgba(229, 57, 53, 0.6); pointer-events: none; }
  @media (min-aspect-ratio: 16/9) { .frame { width: auto; height: 100vh; } }
</style>
