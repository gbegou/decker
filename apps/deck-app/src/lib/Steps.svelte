<script lang="ts">
  // The build steps of the selected element: a row per step with what it
  // changes, quick appear/disappear buttons, and a "move step" that starts
  // from the element's current box so that dragging refines it.
  import { api, type ElementInfo, type Step } from "./api";

  let { element, current, last, onrun, ongoto }: {
    element: ElementInfo;
    /** The step shown on the canvas, and the slide's last step. */
    current: number;
    last: number;
    onrun: (label: string, f: () => Promise<unknown>) => void;
    ongoto: (step: number) => void;
  } = $props();

  const steps = $derived(element.steps);

  function save(next: Step[], label: string) {
    onrun(label, () => api.setSteps(element.index, next));
  }
  function patch(i: number, f: (s: Step) => void, label = "Step changed") {
    const next = steps.map((s) => ({ ...s, set: { ...s.set } }));
    f(next[i]);
    save(next, label);
  }
  function remove(i: number) {
    save(steps.filter((_, k) => k !== i), "Step removed");
  }
  function appear() {
    const at = Math.max(1, current || last + 1);
    const next = steps.filter((s) => !(s.at === 0 && s.set.opacity === 0));
    next.unshift({ at: 0, set: { opacity: 0 } });
    next.push({ at, set: { opacity: 1 }, duration: 0.3 });
    save(next, `Appears at click ${at}`);
    ongoto(at);
  }
  function disappear() {
    const at = Math.max(1, current || last + 1);
    save([...steps, { at, set: { opacity: 0 }, duration: 0.3 }], `Disappears at click ${at}`);
    ongoto(at);
  }
  function moveStep() {
    const at = Math.max(1, current || last + 1);
    const r = element.rect;
    save([...steps, { at, set: { x: r.x, y: r.y, w: r.w, h: r.h }, duration: 0.5 }], `Move step at click ${at}: drag the element to its target`);
    ongoto(at);
  }
  const num = (v: number | undefined) => (v === undefined ? "" : String(Math.round(v * 100) / 100));
  const setNum = (s: Step, key: "x" | "y" | "w" | "h" | "rotation", v: string) => {
    if (v.trim() === "") delete s.set[key]; else s.set[key] = +v;
  };
</script>

<div class="steps">
  <div class="row">
    <button onclick={appear} title="Hidden at first, shown at the current click (or the next new one)">Appear on click</button>
    <button onclick={disappear}>Disappear on click</button>
    <button onclick={moveStep} title="A step whose target box you then drag on the canvas">+ Move step</button>
  </div>
  {#if steps.length === 0}
    <p class="muted">No build steps: the element is shown as is. Steps play on each "next" during the presentation. Use ▶ under the canvas to view a later click and drag the element: that makes a move step.</p>
  {:else}
    {#each steps as s, i}
      <div class="step" class:current={s.at === current}>
        <div class="head">
          <label>click <input type="number" min="0" max="99" value={s.at} onchange={(e) => patch(i, (st) => (st.at = Math.max(0, +e.currentTarget.value)))} /></label>
          <button class="link" onclick={() => ongoto(s.at)} title="Show this step on the canvas">view</button>
          <span class="spacer"></span>
          <button onclick={() => remove(i)} title="Remove this step">×</button>
        </div>
        <div class="grid">
          <label class="wide">visibility
            <select value={s.set.opacity === undefined ? "" : s.set.opacity >= 1 ? "1" : s.set.opacity <= 0 ? "0" : "p"}
                    onchange={(e) => patch(i, (st) => { const v = e.currentTarget.value; if (v === "") delete st.set.opacity; else if (v !== "p") st.set.opacity = +v; })}>
              <option value="">unchanged</option>
              <option value="1">shown</option>
              <option value="0">hidden</option>
              {#if s.set.opacity !== undefined && s.set.opacity > 0 && s.set.opacity < 1}<option value="p">{s.set.opacity}</option>{/if}
            </select>
          </label>
          <label>x <input type="number" value={num(s.set.x)} placeholder="–" onchange={(e) => patch(i, (st) => setNum(st, "x", e.currentTarget.value))} /></label>
          <label>y <input type="number" value={num(s.set.y)} placeholder="–" onchange={(e) => patch(i, (st) => setNum(st, "y", e.currentTarget.value))} /></label>
          <label>w <input type="number" value={num(s.set.w)} placeholder="–" onchange={(e) => patch(i, (st) => setNum(st, "w", e.currentTarget.value))} /></label>
          <label>h <input type="number" value={num(s.set.h)} placeholder="–" onchange={(e) => patch(i, (st) => setNum(st, "h", e.currentTarget.value))} /></label>
          <label>rotation° <input type="number" value={num(s.set.rotation)} placeholder="–" onchange={(e) => patch(i, (st) => setNum(st, "rotation", e.currentTarget.value))} /></label>
          <label>fill
            <input type="color" value={s.set.fill ?? "#000000"} onchange={(e) => patch(i, (st) => (st.set.fill = e.currentTarget.value))} />
            {#if s.set.fill}<button class="link" onclick={() => patch(i, (st) => delete st.set.fill)}>unset</button>{/if}
          </label>
          <label>duration s <input type="number" min="0" step="0.1" value={s.duration ?? 0} onchange={(e) => patch(i, (st) => (st.duration = Math.max(0, +e.currentTarget.value)))} /></label>
          <label>delay s <input type="number" min="0" step="0.1" value={s.delay ?? 0} onchange={(e) => patch(i, (st) => (st.delay = Math.max(0, +e.currentTarget.value)))} /></label>
          <label class="wide">easing
            <select value={s.easing ?? "ease_in_out"} onchange={(e) => patch(i, (st) => (st.easing = e.currentTarget.value as Step["easing"]))}>
              <option value="ease_in_out">ease in out</option><option value="linear">linear</option><option value="ease_in">ease in</option><option value="ease_out">ease out</option>
            </select>
          </label>
        </div>
      </div>
    {/each}
    <p class="muted">Viewing click 1 or later, dragging or resizing the element sets where it is at that click (a move step is created if needed).</p>
  {/if}
</div>

<style>
  .steps { display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; flex-wrap: wrap; gap: 4px; }
  .row button, .head button { font-size: 11px; padding: 2px 6px; }
  .step { min-width: 0; border: 1px solid var(--border); border-radius: 6px; padding: 6px; }
  .step.current { border-color: var(--accent); }
  .head { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
  .head label { display: flex; align-items: center; gap: 4px; color: var(--muted); font-size: 11px; }
  .head input { width: 48px; padding: 2px 4px; }
  .spacer { flex: 1; }
  .link { background: none; border: none; color: var(--accent); padding: 0; font-size: 11px; }
  /* Cells may shrink below their content: the side panel is narrow. */
  .grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 3px 8px; }
  .grid label { display: flex; align-items: center; justify-content: space-between; gap: 4px; min-width: 0; color: var(--muted); font-size: 11px; white-space: nowrap; }
  .grid input[type="number"] { flex: 0 1 64px; width: 0; min-width: 36px; padding: 2px 4px; }
  .grid input[type="color"] { width: 26px; height: 20px; padding: 0; border: 1px solid var(--border); background: none; }
  .grid .wide { grid-column: 1 / -1; }
  .grid select { flex: 0 1 auto; min-width: 0; max-width: 100%; padding: 2px; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 4px; font-size: 11px; }
  .muted { color: var(--muted); font-size: 11px; margin: 2px 0; }
</style>
