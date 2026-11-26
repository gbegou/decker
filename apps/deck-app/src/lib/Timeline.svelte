<script lang="ts">
  // The slide's animation timeline: one row per click, with what changes
  // at that click, element by element. Drag an entry onto another click to
  // move it there, or onto the line between two clicks to give it a click
  // of its own. Clicking an entry selects its element; a click's ▶ shows
  // the slide at that click.
  import type { ElementInfo, Step } from "./api";

  let {
    elements,
    last,
    current,
    selected,
    onselect,
    ongoto,
    onmove,
    ontrigger,
  }: {
    elements: ElementInfo[];
    last: number;
    current: number;
    selected: number | null;
    onselect: (element: number) => void;
    ongoto: (step: number) => void;
    onmove: (element: number, step: number, to: number, insert: boolean, position: number | null) => void;
    ontrigger: (element: number, step: number, mode: "click" | "with" | "after") => void;
  } = $props();

  type Entry = { element: number; step: number; label: string; what: string; seq: number; trigger: "with" | "after"; duration: number };

  function describe(s: Step): string {
    const o = s.set ?? {};
    const parts: string[] = [];
    if (o.opacity !== undefined && o.opacity !== null) parts.push(o.opacity >= 1 ? "appears" : o.opacity <= 0 ? "disappears" : "fades");
    if (o.x != null || o.y != null) parts.push("moves");
    if (o.w != null || o.h != null) parts.push("resizes");
    if (o.rotation != null) parts.push("turns");
    if (o.fill != null) parts.push("changes color");
    return parts.join(", ") || "changes";
  }

  const rows = $derived.by(() => {
    const out: Entry[][] = Array.from({ length: last }, () => []);
    for (const el of elements) {
      if (el.kind === "placeholder") continue;
      el.steps.forEach((s, k) => {
        if (s.at >= 1 && s.at <= last) {
          out[s.at - 1].push({ element: el.index, step: k, label: el.label, what: describe(s), seq: s.seq ?? 0, trigger: s.trigger ?? "with", duration: s.duration ?? 0 });
        }
      });
    }
    // Playing order within a click: its place, then the element's.
    for (const row of out) row.sort((a, b) => a.seq - b.seq || a.element - b.element || a.step - b.step);
    return out;
  });
  /** Elements hidden when the slide opens (they appear later). */
  const hidden = $derived(
    elements.filter((el) => el.steps.some((s) => s.at === 0 && s.set?.opacity === 0)).map((el) => el.label),
  );

  // ----- dragging an entry -----
  let dragging = $state<Entry | null>(null);
  let pressed: { entry: Entry; y: number } | null = null;
  /** Where it would land: a click (`insert` false) or a new click before `at`. */
  let target = $state<{ at: number; insert: boolean; position: number | null } | null>(null);

  function down(e: PointerEvent, entry: Entry) {
    if (e.button !== 0) return;
    pressed = { entry, y: e.clientY };
  }
  function move(e: PointerEvent) {
    if (!pressed) return;
    if (!dragging) {
      if (Math.abs(e.clientY - pressed.y) < 5) return;
      dragging = pressed.entry;
    }
    const under = document.elementFromPoint(e.clientX, e.clientY);
    const hit = under?.closest<HTMLElement>("[data-at]");
    // Over an entry: before it when in its upper half, after it otherwise.
    const over = under?.closest<HTMLElement>("[data-pos]");
    let position: number | null = null;
    if (over) {
      const r = over.getBoundingClientRect();
      position = Number(over.dataset.pos) + (e.clientY > r.top + r.height / 2 ? 1 : 0);
    }
    target = hit ? { at: Number(hit.dataset.at), insert: hit.dataset.insert === "1", position } : null;
  }
  function up() {
    if (dragging && target) {
      const { element, step } = dragging;
      const row = rows[target.at - 1] ?? [];
      const from = row.findIndex((x) => x.element === element && x.step === step);
      let position = target.position;
      // Within its own click, the entry is taken out first: later places shift.
      if (!target.insert && from >= 0 && position !== null && position > from) position -= 1;
      const same = !target.insert && from >= 0 && (position === null ? from === row.length - 1 : position === from);
      if (!same) onmove(element, step, target.at, target.insert, target.insert ? null : position);
    } else if (pressed && !dragging) {
      onselect(pressed.entry.element);
    }
    pressed = null;
    dragging = null;
    target = null;
  }
</script>

<svelte:window onpointermove={move} onpointerup={up} onpointercancel={up} />

<div class="timeline" class:dragging={dragging !== null}>
  <div class="row start" class:current={current === 0}>
    <button class="go" title="The slide as it opens" onclick={() => ongoto(0)}>▶</button>
    <span class="click">Start</span>
    <span class="muted">{hidden.length ? `hidden: ${hidden.join(", ")}` : "everything shown"}</span>
  </div>
  {#each rows as entries, i}
    <div class="gap" data-at={i + 1} data-insert="1" class:over={target?.insert && target.at === i + 1} title="A new click here"></div>
    <div class="row" data-at={i + 1} data-insert="0" class:current={current === i + 1} class:over={target && !target.insert && target.at === i + 1}>
      <button class="go" title="Show the slide at click {i + 1}" onclick={() => ongoto(i + 1)}>▶</button>
      <span class="click">Click {i + 1}</span>
      <div class="entries">
        {#each entries as entry, k (entry.element + ":" + entry.step)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="entry" data-pos={k} class:selected={entry.element === selected} class:moving={dragging === entry}
               class:before={target && !target.insert && target.at === i + 1 && target.position === k}
               class:after={target && !target.insert && target.at === i + 1 && target.position === k + 1 && k === entries.length - 1}
               onpointerdown={(e) => down(e, entry)} title="Drag to reorder: within the click, to another click, or between two clicks for a click of its own">
            <span class="text"><b>{entry.label}</b> {entry.what}{entry.duration ? ` · ${entry.duration} s` : ""}</span>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <select class="when" title="When it starts" value={k === 0 ? (i === 0 && entry.trigger === "after" ? "after" : "click") : entry.trigger}
                    onpointerdown={(e) => e.stopPropagation()}
                    onchange={(e) => ontrigger(entry.element, entry.step, e.currentTarget.value as "click" | "with" | "after")}>
              <option value="click">on click</option>
              {#if k === 0 && i === 0}
                <option value="after" title="Plays by itself when the slide is shown">with the slide</option>
              {:else}
                <option value="with">with previous</option>
                <option value="after">after previous</option>
              {/if}
            </select>
          </div>
        {/each}
      </div>
    </div>
  {/each}
  <div class="gap end" data-at={last + 1} data-insert="1" class:over={target?.insert && target.at === last + 1}>
    {#if dragging}a new click at the end{:else if last > 0}<span class="hint">drag entries to reorder</span>{/if}
  </div>
  {#if last === 0}
    <p class="muted">No animation yet: select an element and add a step below (appear on click, move…).</p>
  {/if}
</div>

<style>
  .timeline { display: flex; flex-direction: column; user-select: none; }
  .row { display: grid; grid-template-columns: auto auto minmax(0, 1fr); gap: 6px; align-items: start; padding: 4px; border: 1px solid var(--border); border-radius: 6px; }
  .row.current { border-color: var(--accent); }
  .row.over { background: rgba(79, 140, 255, 0.15); }
  .row.start { color: var(--muted); }
  .go { font-size: 10px; padding: 1px 5px; }
  .click { font-size: 11px; color: var(--muted); padding-top: 2px; white-space: nowrap; }
  .entries { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .entry { font-size: 12px; padding: 2px 4px 2px 6px; border-radius: 4px; background: var(--bg); cursor: grab; display: flex; flex-wrap: wrap; align-items: center; gap: 2px 4px; border-top: 2px solid transparent; border-bottom: 2px solid transparent; }
  .entry .text { flex: 1 1 100%; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entry .when { flex: none; margin-left: auto; width: 110px; min-width: 0; height: 20px; font-size: 10px; padding: 0 2px; background: var(--panel); color: var(--muted); border: 1px solid var(--border); border-radius: 4px; }
  .entry.before { border-top-color: var(--accent); }
  .entry.after { border-bottom-color: var(--accent); }
  .entry.selected { outline: 1px solid var(--accent); }
  .entry.moving { opacity: 0.4; }
  /* Fixed heights: nothing moves under the pointer when a drag starts. */
  .gap { height: 10px; border-radius: 3px; }
  .gap.over { background: var(--accent); }
  .gap.end { height: 20px; font-size: 11px; color: var(--muted); text-align: center; line-height: 20px; }
  .hint { opacity: 0.6; }
  .muted { color: var(--muted); font-size: 11px; }
</style>
