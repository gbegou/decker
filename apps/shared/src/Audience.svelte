<script lang="ts">
  // The projector window: the current page, letterboxed on black, the
  // blank screen, the presenter's pointer, click to advance. The `page`
  // snippet draws the current page the application's way.
  import { onMount, type Snippet } from "svelte";
  import { show, watchView, presentationKeys, type ShowView } from "./show";
  import { watchPointer, watchInk, inkPoints, type Pointer, type Stroke } from "./pointer";

  let { page }: { page: Snippet<[ShowView]> } = $props();

  let view = $state<ShowView | null>(null);
  let pointer = $state<Pointer | null>(null);
  let ink = $state<Stroke[]>([]);
  onMount(() => {
    const stopView = watchView((v) => (view = v));
    const stopPointer = watchPointer((p) => (pointer = p));
    const stopInk = watchInk((s) => (ink = s));
    return () => { stopView(); stopPointer(); stopInk(); };
  });

  function onKey(e: KeyboardEvent) {
    if (presentationKeys(e)) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="audience" style="background: {view?.blank === 'white' ? 'white' : 'black'}" onclick={() => show.navigate("next")} oncontextmenu={(e) => { e.preventDefault(); show.navigate("prev"); }}>
  {#if view && !view.blank}
    <div class="frame" style="aspect-ratio: {view.width} / {view.height}">
      {@render page(view)}
      {#if ink.length}
        <svg class="ink" viewBox="0 0 1000 1000" preserveAspectRatio="none">
          {#each ink as stroke}<polyline points={inkPoints(stroke)} />{/each}
        </svg>
      {/if}
      {#if pointer}<div class="dot" style="left: {pointer.x * 100}%; top: {pointer.y * 100}%"></div>{/if}
    </div>
  {/if}
</div>

<style>
  .audience { width: 100vw; height: 100vh; background: black; display: flex; align-items: center; justify-content: center; cursor: none; }
  .frame { position: relative; max-width: 100vw; max-height: 100vh; width: 100vw; }
  @media (min-aspect-ratio: 16/9) { .frame { width: auto; height: 100vh; } }
  .ink { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
  .ink polyline { fill: none; stroke: #e53935; stroke-width: 0.6vmin; stroke-linecap: round; stroke-linejoin: round; vector-effect: non-scaling-stroke; }
  .dot { position: absolute; width: 1.6vmin; height: 1.6vmin; margin: -0.8vmin 0 0 -0.8vmin; border-radius: 50%; background: #e53935; box-shadow: 0 0 1vmin 0.4vmin rgba(229, 57, 53, 0.6); pointer-events: none; }
</style>
