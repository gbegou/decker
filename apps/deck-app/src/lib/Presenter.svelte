<script lang="ts">
  // Presenter view: current slide, what comes next, notes, timer.
  import { onMount } from "svelte";
  import { api, watchView, watchThumbnails, presentationKeys, type View } from "./api";
  import SlideView from "./SlideView.svelte";

  let view = $state<View | null>(null);
  let thumbnails = $state<string[]>([]);
  let started = Date.now();
  let now = $state(Date.now());
  let fontSize = $state(20);

  onMount(() => {
    const stop = watchView((v) => (view = v));
    const stopThumbs = watchThumbnails((t) => (thumbnails = t));
    const tick = setInterval(() => (now = Date.now()), 1000);
    return () => { stop(); stopThumbs(); clearInterval(tick); };
  });

  function elapsed(): string {
    const s = Math.floor((now - started) / 1000);
    const m = Math.floor(s / 60);
    return `${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  }

  function onKey(e: KeyboardEvent) {
    if (presentationKeys(e)) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="presenter">
  {#if view}
    <div class="current">
      <div class="label">Current · slide {view.slide + 1} / {view.slide_count}{#if view.in_section} · {view.in_section}{#if view.in_subsection} › {view.in_subsection}{/if}{/if}{#if view.last_step}, step {view.step} / {view.last_step}{/if}{#if view.blank} · {view.blank.toUpperCase()} SCREEN{/if}</div>
      <SlideView svg={view.svg} width={view.width} height={view.height} dir={view.dir} animated />
    </div>
    <div class="side">
      <div class="clock">
        <span class="elapsed">{elapsed()}</span>
        <span class="time">{new Date(now).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
        <button onclick={() => (started = Date.now())}>reset</button>
      </div>
      <div class="next">
        <div class="label">Next</div>
        {#if view.next_svg}
          <SlideView svg={view.next_svg} width={view.width} height={view.height} dir={view.dir} />
        {:else}
          <div class="end">End of presentation</div>
        {/if}
      </div>
      <div class="notes" style="font-size: {fontSize}px">
        <div class="label">Notes
          <button onclick={() => (fontSize = Math.max(12, fontSize - 2))}>A−</button>
          <button onclick={() => (fontSize = fontSize + 2)}>A+</button>
        </div>
        <pre>{view.notes || "—"}</pre>
      </div>
      <div class="controls">
        <button onclick={() => api.navigate("prev")}>◀ Prev</button>
        <button class="primary" onclick={() => api.navigate("next")}>Next ▶</button>
        <button onclick={() => api.navigate("black")}>Black (B)</button>
        <button onclick={() => api.navigate("white")}>White (W)</button>
        <button onclick={() => api.stopPresentation()}>End (Esc)</button>
      </div>
      <div class="strip">
        {#each thumbnails as thumb, i}
          <button class="thumb" class:current={i === view.slide} onclick={() => api.goto(i)}>
            <SlideView svg={thumb} width={view.width} height={view.height} />
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .presenter { display: grid; grid-template-columns: 3fr 2fr; gap: 16px; padding: 16px; height: 100vh; }
  .current { display: flex; flex-direction: column; min-height: 0; }
  .side { display: flex; flex-direction: column; gap: 12px; min-height: 0; }
  .label { color: var(--muted); font-size: 12px; text-transform: uppercase; margin-bottom: 6px; display: flex; gap: 6px; align-items: center; }
  .clock { display: flex; align-items: baseline; gap: 16px; }
  .elapsed { font-size: 40px; font-variant-numeric: tabular-nums; }
  .time { color: var(--muted); font-size: 20px; }
  .notes { flex: 1; overflow-y: auto; }
  .notes pre { white-space: pre-wrap; font-family: inherit; margin: 0; }
  .controls { display: flex; gap: 8px; }
  .strip { display: flex; gap: 6px; overflow-x: auto; }
  .thumb { width: 90px; flex: none; padding: 2px; }
  .thumb.current { border-color: var(--accent); }
  .end { color: var(--muted); padding: 24px; text-align: center; border: 1px dashed var(--border); }
</style>
