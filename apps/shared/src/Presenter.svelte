<script lang="ts" generics="T">
  // The presenter window: current page, what comes next, notes, clock
  // and elapsed time, the pointer, controls and a strip of thumbnails.
  // The application draws pages through the `page`, `next` and `thumb`
  // snippets; `thumbs` is whatever its thumbnails are (SVG strings,
  // page numbers).
  import { onMount, type Snippet } from "svelte";
  import { show, watchView, presentationKeys, type ShowView } from "./show";
  import { sendPointer, pointerIn, sendInk, inkPoints, type Pointer, type Stroke } from "./pointer";

  let { page, next, thumb, thumbs = [] }: {
    page: Snippet<[ShowView]>;
    next: Snippet<[ShowView]>;
    thumb: Snippet<[T, number, ShowView]>;
    thumbs?: T[];
  } = $props();

  let view = $state<ShowView | null>(null);
  let started = Date.now();
  let now = $state(Date.now());
  let fontSize = $state(20);
  // The pointer: on, the mouse over the current page is mirrored on the audience window.
  let pointerOn = $state(false);
  let pointer = $state<Pointer | null>(null);
  /** The dot's place in the stage, in pixels: the page box may be narrower than the stage. */
  let dot = $state<{ left: number; top: number } | null>(null);
  let current = $state<HTMLDivElement>();
  let stage = $state<HTMLDivElement>();

  onMount(() => {
    const stop = watchView((v) => (view = v));
    const tick = setInterval(() => (now = Date.now()), 1000);
    return () => { stop(); clearInterval(tick); };
  });

  function elapsed(): string {
    const s = Math.floor((now - started) / 1000);
    const m = Math.floor(s / 60);
    return `${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  }
  // The pen: on, dragging over the current page draws strokes, mirrored
  // on the audience window; they go away with the page (or with C).
  let penOn = $state(false);
  let ink = $state<Stroke[]>([]);
  let drawing = false;
  /** Where the page sits in the stage, in pixels, for the ink overlay. */
  let pageBox = $state<{ left: number; top: number; width: number; height: number } | null>(null);
  let inkPage = "";

  function measurePage(): Element | null {
    const box = current?.querySelector(".page, .slide-svg") ?? null;
    if (box && stage) {
      const r = box.getBoundingClientRect(), s = stage.getBoundingClientRect();
      pageBox = { left: r.left - s.left, top: r.top - s.top, width: r.width, height: r.height };
    }
    return box;
  }
  function clearInk() {
    if (ink.length) { ink = []; sendInk([]); }
  }
  function togglePen() {
    penOn = !penOn;
    if (penOn) { pointerOn = false; hidePointer(); measurePage(); }
  }
  $effect(() => {
    // A new page (or step) starts clean.
    const key = view ? `${view.slide}:${view.step}` : "";
    if (key !== inkPage) { inkPage = key; clearInk(); }
  });
  function penDown(e: PointerEvent) {
    if (!penOn) return;
    const p = pointerIn(e, measurePage());
    if (!p) return;
    drawing = true;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    ink = [...ink, [[p.x, p.y]]];
    sendInk(ink);
    e.preventDefault();
  }
  function penUp() {
    drawing = false;
  }

  function movePointer(e: PointerEvent) {
    if (penOn) {
      if (!drawing) return;
      const p = pointerIn(e, measurePage());
      if (!p) return;
      const last = ink[ink.length - 1];
      const [lx, ly] = last[last.length - 1];
      // A point every half percent of the page is plenty.
      if (Math.hypot(p.x - lx, p.y - ly) < 0.004) return;
      last.push([p.x, p.y]);
      ink = [...ink];
      sendInk(ink);
      return;
    }
    if (!pointerOn) return;
    const box = current?.querySelector(".page, .slide-svg") ?? null;
    pointer = pointerIn(e, box);
    if (pointer && box && stage) {
      const r = box.getBoundingClientRect(), s = stage.getBoundingClientRect();
      dot = { left: r.left - s.left + pointer.x * r.width, top: r.top - s.top + pointer.y * r.height };
    } else dot = null;
    sendPointer(pointer);
  }
  function hidePointer() {
    if (pointer) { pointer = null; dot = null; sendPointer(null); }
  }
  function togglePointer() {
    pointerOn = !pointerOn;
    if (pointerOn) penOn = false;
    if (!pointerOn) hidePointer();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "p" || e.key === "P") { togglePointer(); e.preventDefault(); return; }
    if (e.key === "d" || e.key === "D") { togglePen(); e.preventDefault(); return; }
    if (e.key === "c" || e.key === "C") { clearInk(); e.preventDefault(); return; }
    if (presentationKeys(e)) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="presenter">
  {#if view}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="current" class:pointing={pointerOn || penOn} bind:this={current} onpointermove={movePointer} onpointerleave={hidePointer}
         onpointerdown={penDown} onpointerup={penUp} onpointercancel={penUp}>
      <div class="label">Current · {view.slide + 1} / {view.slide_count}{#if view.in_section} · {view.in_section}{#if view.in_subsection} › {view.in_subsection}{/if}{/if}{#if view.last_step}, step {view.step} / {view.last_step}{/if}{#if view.blank} · {view.blank.toUpperCase()} SCREEN{/if}</div>
      <div class="stage" bind:this={stage}>
        {@render page(view)}
        {#if ink.length && pageBox}
          <svg class="ink" viewBox="0 0 1000 1000" preserveAspectRatio="none"
               style="left: {pageBox.left}px; top: {pageBox.top}px; width: {pageBox.width}px; height: {pageBox.height}px">
            {#each ink as stroke}<polyline points={inkPoints(stroke)} />{/each}
          </svg>
        {/if}
        {#if dot}<div class="dot" style="left: {dot.left}px; top: {dot.top}px"></div>{/if}
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
        {#if view.next_svg !== null || view.slide + 1 < view.slide_count || view.step < view.last_step}
          {@render next(view)}
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
        <button onclick={() => show.navigate("prev")}>◀ Prev</button>
        <button class="primary" onclick={() => show.navigate("next")}>Next ▶</button>
        <button onclick={() => show.navigate("black")}>Black (B)</button>
        <button onclick={() => show.navigate("white")}>White (W)</button>
        <button class:on={pointerOn} onclick={togglePointer} title="Mirror the mouse over the current page on the audience window (P)">Pointer (P)</button>
        <button class:on={penOn} onclick={togglePen} title="Draw on the current page; the audience sees it. Gone with the page, or with C (D)">Pen (D)</button>
        {#if ink.length}<button onclick={clearInk} title="Erase the drawing (C)">Clear (C)</button>{/if}
        <button onclick={() => show.stopPresentation()}>End (Esc)</button>
      </div>
      <div class="strip">
        {#each thumbs as t, i}
          <button class="thumb" class:current={i === view.slide} onclick={() => show.goto(i)}>
            {@render thumb(t, i, view)}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  /* minmax(0, …): a long thumbnail strip must scroll, not widen its column. */
  .presenter { display: grid; grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); gap: 16px; padding: 16px; height: 100vh; }
  .current { display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  .current.pointing { cursor: crosshair; }
  /* A flex column with a definite height, so that a page's max-height: 100% fits it. */
  .stage { position: relative; width: 100%; flex: 1; min-height: 0; display: flex; flex-direction: column; align-items: flex-start; }
  .dot { position: absolute; width: 14px; height: 14px; margin: -7px 0 0 -7px; border-radius: 50%; background: #e53935; box-shadow: 0 0 6px 2px rgba(229, 57, 53, 0.6); pointer-events: none; }
  button.on { border-color: var(--accent); color: var(--accent); }
  .ink { position: absolute; pointer-events: none; }
  .ink polyline { fill: none; stroke: #e53935; stroke-width: 3px; stroke-linecap: round; stroke-linejoin: round; vector-effect: non-scaling-stroke; }
  .side { display: flex; flex-direction: column; gap: 12px; min-height: 0; min-width: 0; }
  .label { color: var(--muted); font-size: 12px; text-transform: uppercase; margin-bottom: 6px; display: flex; gap: 6px; align-items: center; }
  .clock { display: flex; align-items: baseline; gap: 16px; }
  .elapsed { font-size: 40px; font-variant-numeric: tabular-nums; }
  .time { color: var(--muted); font-size: 20px; }
  .notes { flex: 1; overflow-y: auto; }
  .notes pre { white-space: pre-wrap; font-family: inherit; margin: 0; }
  .controls { display: flex; gap: 8px; flex-wrap: wrap; }
  .strip { display: flex; gap: 6px; overflow-x: auto; }
  .thumb { width: 90px; flex: none; padding: 2px; }
  .thumb.current { border-color: var(--accent); }
  .end { color: var(--muted); padding: 24px; text-align: center; border: 1px dashed var(--border); }
</style>
