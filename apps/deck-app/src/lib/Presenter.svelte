<script lang="ts">
  // Presenter view: current slide, what comes next, notes, timer.
  import { onMount } from "svelte";
  import { api, watchView, watchThumbnails, presentationKeys, type View } from "./api";
  import SlideView from "./SlideView.svelte";
  import { sendPointer, pointerIn, type Pointer } from "./pointer";

  let view = $state<View | null>(null);
  // The pointer: on, the mouse over the current slide is mirrored on the audience window.
  let pointerOn = $state(false);
  let pointer = $state<Pointer | null>(null);
  let current = $state<HTMLDivElement>();
  function movePointer(e: PointerEvent) {
    if (!pointerOn) return;
    pointer = pointerIn(e, current?.querySelector(".slide-svg") ?? null);
    sendPointer(pointer);
  }
  function hidePointer() {
    if (pointer) { pointer = null; sendPointer(null); }
  }
  function togglePointer() {
    pointerOn = !pointerOn;
    if (!pointerOn) hidePointer();
  }
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
    if (e.key === "p" || e.key === "P") { togglePointer(); e.preventDefault(); return; }
    if (presentationKeys(e)) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="presenter">
  {#if view}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="current" class:pointing={pointerOn} bind:this={current} onpointermove={movePointer} onpointerleave={hidePointer}>
      <div class="label">Current · slide {view.slide + 1} / {view.slide_count}{#if view.in_section} · {view.in_section}{#if view.in_subsection} › {view.in_subsection}{/if}{/if}{#if view.last_step}, step {view.step} / {view.last_step}{/if}{#if view.blank} · {view.blank.toUpperCase()} SCREEN{/if}</div>
      <div class="stage">
        <SlideView svg={view.svg} width={view.width} height={view.height} dir={view.dir} animated />
        {#if pointer}<div class="dot" style="left: {pointer.x * 100}%; top: {pointer.y * 100}%"></div>{/if}
      </div>
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
        <button class:on={pointerOn} onclick={togglePointer} title="Mirror the mouse over the current slide on the audience window (P)">Pointer (P)</button>
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
  .current.pointing { cursor: crosshair; }
  .stage { position: relative; display: inline-block; max-width: 100%; }
  .dot { position: absolute; width: 14px; height: 14px; margin: -7px 0 0 -7px; border-radius: 50%; background: #e53935; box-shadow: 0 0 6px 2px rgba(229, 57, 53, 0.6); pointer-events: none; }
  button.on { border-color: var(--accent); color: var(--accent); }
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
