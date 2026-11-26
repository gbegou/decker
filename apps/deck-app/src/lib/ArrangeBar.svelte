<script lang="ts">
  // The arrange toolbar, shown under the style bar for any selection:
  // align (to the selection, or to the slide for one element),
  // distribute, mirror, drawing order, and for shapes the outline's
  // color and width.
  import { api, type ElementInfo } from "./api";

  let { elements, onrun, onmoved }: {
    /** The selected elements, the primary one first. */
    elements: ElementInfo[];
    onrun: (label: string, f: () => Promise<unknown>) => void;
    /** Called with the selection's indices after a reordering. */
    onmoved: (indices: number[]) => void;
  } = $props();

  const indices = $derived(elements.map((e) => e.index));
  const one = $derived(elements.length === 1 ? elements[0] : null);
  const stroked = $derived(elements.filter((e) => e.kind === "shape" || e.kind === "text" || e.kind === "table"));

  function arrange(op: string, label: string) {
    onrun(label, async () => { onmoved(await api.arrange(indices, op)); });
  }
  function stroke(change: { stroke?: string; stroke_width?: number }, label: string) {
    onrun(label, async () => { for (const e of stroked) await api.styleElement(e.index, change); });
  }
  const ALIGN: [string, string, string][] = [
    ["align_left", "Align left", "M4 3v18M8 7h10v4H8zM8 14h6v4H8z"],
    ["align_center", "Align centers horizontally", "M12 3v18M6 7h12v4H6zM8 14h8v4H8z"],
    ["align_right", "Align right", "M20 3v18M6 7h10v4H6zM10 14h6v4h-6z"],
    ["align_top", "Align top", "M3 4h18M7 8h4v10H7zM14 8h4v6h-4z"],
    ["align_middle", "Align centers vertically", "M3 12h18M7 6h4v12H7zM14 8h4v8h-4z"],
    ["align_bottom", "Align bottom", "M3 20h18M7 6h4v10H7zM14 10h4v6h-4z"],
  ];
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="arrangebar" onpointerdown={(e) => e.stopPropagation()}>
  <span class="scope" title="Alignment is to the other selected elements, or to the slide for one element">Arrange · {elements.length > 1 ? `${elements.length} items` : "on the slide"}</span>
  {#each ALIGN as [op, title, d]}
    <button {title} aria-label={title} onclick={() => arrange(op, title)}><svg viewBox="0 0 24 24"><path {d} /></svg></button>
  {/each}
  <span class="sep"></span>
  <button title="Distribute horizontally: equal gaps (3 or more)" aria-label="Distribute horizontally" disabled={elements.length < 3} onclick={() => arrange("distribute_h", "Distributed")}>
    <svg viewBox="0 0 24 24"><path d="M3 3v18M21 3v18M7 8h3v8H7zM14 8h3v8h-3z" /></svg></button>
  <button title="Distribute vertically: equal gaps (3 or more)" aria-label="Distribute vertically" disabled={elements.length < 3} onclick={() => arrange("distribute_v", "Distributed")}>
    <svg viewBox="0 0 24 24"><path d="M3 3h18M3 21h18M8 7h8v3H8zM8 14h8v3H8z" /></svg></button>
  <span class="sep"></span>
  <button title="Mirror left to right" aria-label="Mirror horizontally" onclick={() => arrange("mirror_h", "Mirrored")}>
    <svg viewBox="0 0 24 24"><path d="M12 2v20M9 6L3 18h6zM15 6l6 12h-6z" class="half" /></svg></button>
  <button title="Mirror top to bottom" aria-label="Mirror vertically" onclick={() => arrange("mirror_v", "Mirrored")}>
    <svg viewBox="0 0 24 24"><path d="M2 12h20M6 9L18 3v6zM6 15l12 6v-6z" class="half" /></svg></button>
  <span class="sep"></span>
  <button title="Bring to front" aria-label="Bring to front" onclick={() => arrange("front", "To front")}>⤒</button>
  <button title="Bring forward" aria-label="Bring forward" onclick={() => arrange("forward", "Forward")}>↑</button>
  <button title="Send backward" aria-label="Send backward" onclick={() => arrange("backward", "Backward")}>↓</button>
  <button title="Send to back" aria-label="Send to back" onclick={() => arrange("back", "To back")}>⤓</button>
  {#if stroked.length}
    <span class="sep"></span>
    <label title="Stroke color">stroke
      <input type="color" value={one?.stroke?.color ?? stroked[0].stroke?.color ?? "#000000"} onchange={(e) => stroke({ stroke: e.currentTarget.value }, "Stroke color")} />
    </label>
    <input type="number" title="Stroke width in points" min="0.25" max="40" step="0.25" value={one?.stroke?.width ?? stroked[0].stroke?.width ?? ""} placeholder="pt"
           onchange={(e) => stroke({ stroke_width: +e.currentTarget.value }, "Stroke width")} />
    <button title="No stroke" onclick={() => stroke({ stroke: "" }, "Stroke removed")}>⌀</button>
  {/if}
</div>

<style>
  .arrangebar { display: flex; flex-wrap: nowrap; flex: none; align-items: center; gap: 4px; padding: 4px 8px; font-size: 12px; color: var(--muted); white-space: nowrap; border-left: 1px solid var(--border); }
  .scope { min-width: 0; }
  button { padding: 2px 5px; min-width: 26px; height: 24px; display: inline-flex; align-items: center; justify-content: center; }
  button:disabled { opacity: 0.35; cursor: default; }
  svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linejoin: round; }
  .sep { width: 1px; height: 18px; background: var(--border); margin: 0 4px; }
  input[type="color"] { width: 28px; height: 22px; padding: 0; border: 1px solid var(--border); background: none; }
  input[type="number"] { width: 56px; padding: 2px 4px; }
  label { display: inline-flex; align-items: center; gap: 4px; }
</style>
