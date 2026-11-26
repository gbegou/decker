<script lang="ts">
  // The build steps of the selected element: a row per step with what it
  // changes, quick appear/disappear buttons, and a "move step" that starts
  // from the element's current box so that dragging refines it.
  import { api, type ElementInfo, type Step } from "./api";

  let { element, current, last, onrun, ongoto, firstOfClick = () => true }: {
    element: ElementInfo;
    /** Whether a step is the first effect of its click (it starts on the click). */
    firstOfClick?: (s: Step) => boolean;
    /** The step shown on the canvas, and the slide's last step. */
    current: number;
    last: number;
    onrun: (label: string, f: () => Promise<unknown>) => void;
    ongoto: (step: number) => void;
  } = $props();

  const steps = $derived(element.steps);
  /** The hide at the start that "Appear on click" adds: implied, not listed. */
  const auto = (s: Step) => s.at === 0 && s.set.opacity === 0 && Object.keys(s.set).length === 1;
  function describe(s: Step): string {
    const o = s.set, parts: string[] = [];
    if (o.opacity !== undefined) parts.push(o.opacity >= 1 ? "appears" : o.opacity <= 0 ? "disappears" : "fades");
    if (o.x !== undefined || o.y !== undefined) parts.push("moves");
    if (o.w !== undefined || o.h !== undefined) parts.push("resizes");
    if (o.rotation !== undefined) parts.push("turns");
    if (o.fill !== undefined) parts.push("changes color");
    return parts.join(", ") || "no change";
  }

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
    <p class="muted" title="Steps play on each click of the presentation. Viewing a click (▶ under the slide), dragging the element makes a move step.">No animation: shown as is.</p>
  {:else}
    {#if steps.some(auto)}
      <p class="muted">Hidden when the slide opens, until it appears.</p>
    {/if}
    {#each steps as s, i}
      {#if !auto(s)}
      <details class="step" class:current={s.at === current}>
        <summary>
          <span class="what">{s.at === 0 ? "Start" : `Click ${s.at}`} · {describe(s)}{s.duration ? ` · ${s.duration} s` : ""}</span>
          <button class="link" onclick={(e) => { e.preventDefault(); ongoto(s.at); }} title="Show this click on the canvas">view</button>
          <button onclick={(e) => { e.preventDefault(); remove(i); }} title="Remove this step">×</button>
        </summary>
        {#if s.at > 0}
          <label class="line">starts
            <select value={firstOfClick(s) ? "click" : (s.trigger ?? "with")}
                    onchange={(e) => { const mode = e.currentTarget.value as "click" | "with" | "after"; onrun("Start of the effect", () => api.setTrigger(element.index, i, mode)); }}>
              <option value="click">on click</option>
              <option value="with">with previous</option>
              <option value="after">after previous</option>
            </select>
          </label>
        {/if}
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
      </details>
      {/if}
    {/each}
    <p class="muted" title="A move step is created if the element has none at that click.">Viewing a click, dragging the element sets where it is at that click.</p>
  {/if}
</div>

<style>
  .steps { display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; flex-wrap: wrap; gap: 4px; }
  .row button { font-size: 11px; padding: 2px 6px; }
  details.step > summary { display: flex; align-items: center; gap: 6px; cursor: pointer; font-size: 12px; list-style: none; }
  details.step > summary::before { content: "▸"; color: var(--muted); }
  details.step[open] > summary::before { content: "▾"; }
  details.step .what { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  details.step > summary button { font-size: 11px; padding: 0 5px; }
  details.step[open] > summary { margin-bottom: 6px; }
  .line { display: flex; align-items: center; justify-content: space-between; gap: 6px; color: var(--muted); font-size: 11px; margin-bottom: 4px; }
  .line select { padding: 2px; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 4px; font-size: 11px; }
  .step { min-width: 0; border: 1px solid var(--border); border-radius: 6px; padding: 6px; }
  .step.current { border-color: var(--accent); }
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
