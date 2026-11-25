<script lang="ts">
  // The style bar above the canvas: run and paragraph styles of a text
  // element (the selection while editing, else the whole element), and
  // the colors and sizes of shapes, formulas and the plan.
  import { api, type ElementInfo, type StyleInfo, type StyleChange, type ParagraphChange, type Align, type ThemeInfo } from "./api";

  let { element, range, theme, onrun }: {
    element: ElementInfo;
    /** The caret selection while editing, else null for the whole element. */
    range: { from: number; to: number } | null;
    /** The theme in force: its palette becomes swatches, its fonts the font list. */
    theme: ThemeInfo;
    onrun: (label: string, f: () => Promise<unknown>) => void;
  } = $props();

  let info = $state<StyleInfo | null>(null);
  const textual = $derived(element.kind === "text" || element.kind === "shape");
  const FALLBACK_FONTS = ["DejaVu Sans", "Libertinus Serif", "DejaVu Sans Mono", "New Computer Modern"];
  const fonts = $derived(theme.fonts.length ? theme.fonts : FALLBACK_FONTS);
  /** The accent then the palette, without repeats. */
  const swatches = $derived([...new Set([theme.effective.accent, ...(theme.effective.palette ?? [])].filter((c): c is string => !!c))]);

  $effect(() => {
    // Re-read the style whenever the element or the selection changes.
    const id = element.index, r = range, k = element.kind, t = element.text;
    void t;
    if (k === "text" || k === "shape") {
      api.styleInfo(id, r?.from ?? null, r?.to ?? null).then((i) => (info = i)).catch(() => (info = null));
    } else info = null;
  });

  function runStyle(change: StyleChange, label: string) {
    onrun(label, () => api.styleText(element.index, range?.from ?? null, range?.to ?? null, change));
  }
  function paraStyle(change: ParagraphChange, label: string) {
    onrun(label, () => api.styleParagraphs(element.index, range?.from ?? null, range?.to ?? null, change));
  }
  function elementStyle(change: Parameters<typeof api.styleElement>[1], label: string) {
    onrun(label, () => api.styleElement(element.index, change));
  }
  const hex = (c: string | null | undefined, fallback: string) => c ?? fallback;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="stylebar" onpointerdown={(e) => e.stopPropagation()}>
  {#if textual && info}
    <span class="scope">{range && range.from !== range.to ? "selection" : range ? "caret" : "whole box"}</span>
    <button class:on={info.bold} title="Bold (Ctrl+B)" onclick={() => runStyle({ toggle_bold: true }, "Bold")}><b>B</b></button>
    <button class:on={info.italic} title="Italic (Ctrl+I)" onclick={() => runStyle({ toggle_italic: true }, "Italic")}><i>I</i></button>
    <label title="Size in points, 0 for the default">
      <input type="number" min="0" max="200" step="1" value={info.size ?? ""} placeholder="18"
             onchange={(e) => runStyle({ size: +e.currentTarget.value || 0 }, "Size")} />
    </label>
    <label title="Text color">
      <input type="color" value={hex(info.color, "#000000")} onchange={(e) => runStyle({ color: e.currentTarget.value }, "Color")} />
    </label>
    <button title="Default color" onclick={() => runStyle({ color: "" }, "Color reset")}>⌀</button>
    {#each swatches as c}
      <button class="swatch" style="background:{c}" title="Theme color {c}" aria-label="Theme color {c}" onclick={() => runStyle({ color: c }, "Color")}></button>
    {/each}
    <select title="Font" value={info.font ?? ""} onchange={(e) => runStyle({ font: e.currentTarget.value }, "Font")}>
      <option value="">theme font{theme.effective.font ? ` (${theme.effective.font})` : ""}</option>
      {#each fonts as f}<option value={f}>{f}</option>{/each}
    </select>
    <span class="sep"></span>
    {#each [["left", "⯇"], ["center", "☰"], ["right", "⯈"], ["justify", "≡"]] as [a, glyph]}
      <button class:on={info.align === a} title={a} onclick={() => paraStyle({ align: a as Align }, "Align")}>{glyph}</button>
    {/each}
    <span class="sep"></span>
    <button class:on={info.bullet !== null} title="Bullet on or off" onclick={() => paraStyle({ bullet: info!.bullet === null ? 0 : -1 }, "Bullet")}>•</button>
    <button title="Outdent" disabled={info.bullet === null || info.bullet === 0} onclick={() => paraStyle({ bullet: (info!.bullet ?? 1) - 1 }, "Outdent")}>←</button>
    <button title="Indent" disabled={info.bullet === null} onclick={() => paraStyle({ bullet: (info!.bullet ?? 0) + 1 }, "Indent")}>→</button>
    <span class="sep"></span>
  {/if}
  {#if element.kind === "text" || element.kind === "shape"}
    <label title="Fill color">fill
      <input type="color" value={hex(element.fill, "#ffffff")} onchange={(e) => elementStyle({ fill: e.currentTarget.value }, "Fill")} />
    </label>
    <button title="No fill" onclick={() => elementStyle({ fill: "" }, "Fill removed")}>⌀</button>
    <label title="Outline color">outline
      <input type="color" value={hex(element.stroke?.color, "#000000")} onchange={(e) => elementStyle({ stroke: e.currentTarget.value }, "Outline")} />
    </label>
    <input type="number" title="Outline width in points" min="0.25" max="20" step="0.25" value={element.stroke?.width ?? ""} placeholder="w"
           class="narrow" onchange={(e) => elementStyle({ stroke_width: +e.currentTarget.value }, "Outline width")} />
    <button title="No outline" onclick={() => elementStyle({ stroke: "" }, "Outline removed")}>⌀</button>
  {:else if element.kind === "math"}
    <label>size <input type="number" min="0" max="200" value={element.size} class="narrow" onchange={(e) => elementStyle({ size: +e.currentTarget.value || 0 }, "Size")} /></label>
    <label>color <input type="color" value={hex(element.fill, "#000000")} onchange={(e) => elementStyle({ fill: e.currentTarget.value }, "Color")} /></label>
  {:else if element.kind === "table"}
    <label>text size <input type="number" min="0" max="200" value={element.size} class="narrow" onchange={(e) => elementStyle({ size: +e.currentTarget.value || 0 }, "Size")} /></label>
  {:else if element.kind === "outline"}
    <label>size <input type="number" min="0" max="200" value={element.size} class="narrow" onchange={(e) => elementStyle({ size: +e.currentTarget.value || 0 }, "Size")} /></label>
    <label>others <input type="color" value={hex(element.fill, "#888888")} onchange={(e) => elementStyle({ fill: e.currentTarget.value }, "Color")} /></label>
    <label>current <input type="color" value={hex(element.highlight, "#000000")} onchange={(e) => elementStyle({ highlight: e.currentTarget.value }, "Highlight")} /></label>
    <label><input type="checkbox" checked={element.subsections ?? true} onchange={(e) => elementStyle({ subsections: e.currentTarget.checked }, "Subsections")} /> subsections</label>
  {/if}
</div>

<style>
  .stylebar { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; padding: 4px 16px; border-bottom: 1px solid var(--border); font-size: 12px; color: var(--muted); }
  .stylebar button { padding: 2px 7px; min-width: 26px; }
  .stylebar button.on { border-color: var(--accent); color: var(--accent); }
  .stylebar input[type="number"] { width: 58px; padding: 2px 4px; }
  .stylebar input.narrow { width: 52px; }
  .stylebar input[type="color"] { width: 28px; height: 22px; padding: 0; border: 1px solid var(--border); background: none; }
  .stylebar button.swatch { width: 18px; min-width: 18px; height: 18px; padding: 0; border-radius: 50%; border: 1px solid var(--border); }
  .stylebar select { padding: 2px 4px; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; }
  .stylebar label { display: inline-flex; align-items: center; gap: 3px; }
  .scope { padding-right: 4px; }
  .sep { width: 1px; height: 18px; background: var(--border); margin: 0 4px; }
</style>
