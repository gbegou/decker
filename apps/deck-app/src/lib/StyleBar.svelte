<script lang="ts">
  // The style bar above the canvas: run and paragraph styles of a text
  // element (the selection while editing, else the whole element), and
  // the colors and sizes of shapes, formulas and the plan.
  import { api, type ElementInfo, type StyleInfo, type StyleChange, type ParagraphChange, type Align, type ThemeInfo, type VAlign } from "./api";

  let { element, range, theme, onrun }: {
    element: ElementInfo;
    /** The caret selection while editing, else null for the whole element. */
    range: { from: number; to: number } | null;
    /** The theme in force: its palette becomes swatches, its fonts the font list. */
    theme: ThemeInfo;
    onrun: (label: string, f: () => Promise<unknown>) => void;
  } = $props();

  let info = $state<StyleInfo | null>(null);
  // A table takes run styles for the cell being edited in place.
  const textual = $derived(element.kind === "text" || element.kind === "shape" || (element.kind === "table" && range !== null));
  const FALLBACK_FONTS = ["DejaVu Sans", "Libertinus Serif", "DejaVu Sans Mono", "New Computer Modern"];
  const fonts = $derived(theme.fonts.length ? theme.fonts : FALLBACK_FONTS);
  /** The accent then the palette, without repeats. */
  const swatches = $derived([...new Set([theme.effective.accent, ...(theme.effective.palette ?? [])].filter((c): c is string => !!c))]);

  $effect(() => {
    // Re-read the style whenever the element or the selection changes.
    const id = element.index, r = range, k = element.kind, t = element.text;
    void t;
    if (k === "text" || k === "shape" || (k === "table" && r !== null)) {
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

  // Alignment icons: lines of text within a 14×14 box.
  const svg = (body: string) => `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">${body}</svg>`;
  const lines = (xs: [number, number][], ys = [3, 6, 9, 12]) => xs.map(([a, b], i) => `<line x1="${a}" y1="${ys[i]}" x2="${b}" y2="${ys[i]}"/>`).join("");
  const H_ICONS: Record<string, string> = {
    left: svg(lines([[1, 13], [1, 9], [1, 12], [1, 7]])),
    center: svg(lines([[1, 13], [3, 11], [2, 12], [4, 10]])),
    right: svg(lines([[1, 13], [5, 13], [2, 13], [7, 13]])),
    justify: svg(lines([[1, 13], [1, 13], [1, 13], [1, 8]])),
  };
  // A box, and two lines of text at its top, middle or bottom.
  const box = `<rect x="1" y="1" width="12" height="12" rx="1.5" stroke-width="1.1" opacity="0.6"/>`;
  const V_ICONS: Record<string, string> = {
    top: svg(box + lines([[4, 10], [4, 10]], [4, 6.5])),
    middle: svg(box + lines([[4, 10], [4, 10]], [5.8, 8.3])),
    bottom: svg(box + lines([[4, 10], [4, 10]], [7.5, 10])),
  };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="stylebar" onpointerdown={(e) => e.stopPropagation()}>
  {#if textual && info}
    <span class="scope" title="What the text buttons act on: the selected characters, the caret, or the whole box">{range && range.from !== range.to ? "Sel." : range ? "Caret" : "Box"}</span>
    <button class:on={info.bold} title="Bold (Ctrl+B)" onclick={() => runStyle({ toggle_bold: true }, "Bold")}><b>B</b></button>
    <button class:on={info.italic} title="Italic (Ctrl+I)" onclick={() => runStyle({ toggle_italic: true }, "Italic")}><i>I</i></button>
    <label title="Size in points, 0 for the default">
      <input type="number" min="0" max="200" step="1" value={info.size ?? ""} placeholder="18"
             onchange={(e) => runStyle({ size: +e.currentTarget.value || 0 }, "Size")} />
    </label>
    <label title="Text color">
      <input type="color" value={hex(info.color, "#000000")} onchange={(e) => runStyle({ color: e.currentTarget.value }, "Color")} />
    </label>
    <button class="compact" title="Default color" onclick={() => runStyle({ color: "" }, "Color reset")}>⌀</button>
    {#each swatches as c}
      <button class="swatch" style="background:{c}" title="Theme color {c}" aria-label="Theme color {c}" onclick={() => runStyle({ color: c }, "Color")}></button>
    {/each}
    <select title="Font" value={info.font ?? ""} onchange={(e) => runStyle({ font: e.currentTarget.value }, "Font")}>
      <option value="">theme font{theme.effective.font ? ` (${theme.effective.font})` : ""}</option>
      {#each fonts as f}<option value={f}>{f}</option>{/each}
    </select>
    <span class="sep"></span>
    {#each [["left", "Align left"], ["center", "Center"], ["right", "Align right"], ["justify", "Justify"]] as [a, what]}
      <button class="icon" class:on={info.align === a} title="{what} (the paragraphs)" aria-label={what}
              onclick={() => paraStyle({ align: a as Align }, "Align")}>{@html H_ICONS[a]}</button>
    {/each}
    {#if element.kind === "text" || element.kind === "shape"}
      <span class="sep"></span>
      {#each [["top", "Top"], ["middle", "Middle"], ["bottom", "Bottom"]] as [v, what]}
        <button class="icon" class:on={element.valign === v} title="Text at the {what.toLowerCase()} of the box" aria-label="{what} of the box"
                onclick={() => elementStyle({ valign: v as VAlign }, "Vertical alignment")}>{@html V_ICONS[v]}</button>
      {/each}
    {/if}
    <span class="sep"></span>
    <button class="compact" class:on={info.bullet !== null} title="Bullet on or off" onclick={() => paraStyle({ bullet: info!.bullet === null ? 0 : -1 }, "Bullet")}>•</button>
    <button class="compact" title="Outdent" disabled={info.bullet === null || info.bullet === 0} onclick={() => paraStyle({ bullet: (info!.bullet ?? 1) - 1 }, "Outdent")}>←</button>
    <button class="compact" title="Indent" disabled={info.bullet === null} onclick={() => paraStyle({ bullet: (info!.bullet ?? 0) + 1 }, "Indent")}>→</button>
    <span class="sep"></span>
  {/if}
  {#if element.kind === "text" || element.kind === "shape"}
    <label title="Fill color of the box">Fill
      <input type="color" value={hex(element.fill, "#ffffff")} onchange={(e) => elementStyle({ fill: e.currentTarget.value }, "Fill")} />
    </label>
    <button class="compact" title="No fill" onclick={() => elementStyle({ fill: "" }, "Fill removed")}>⌀</button>
    <label title="Outline color of the box">Line
      <input type="color" value={hex(element.stroke?.color, "#000000")} onchange={(e) => elementStyle({ stroke: e.currentTarget.value }, "Outline")} />
    </label>
    <input type="number" title="Outline width in points" min="0.25" max="20" step="0.25" value={element.stroke?.width ?? ""} placeholder="pt"
           class="narrow" onchange={(e) => elementStyle({ stroke_width: +e.currentTarget.value }, "Outline width")} />
    <button class="compact" title="No outline" onclick={() => elementStyle({ stroke: "" }, "Outline removed")}>⌀</button>
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
  .stylebar { display: flex; flex-wrap: nowrap; flex: none; align-items: center; gap: 4px; padding: 4px 8px; font-size: 12px; color: var(--muted); white-space: nowrap; }
  .stylebar button { padding: 2px 7px; min-width: 26px; }
  .stylebar button.on { border-color: var(--accent); color: var(--accent); }
  /* Compact: the row must fit without scrolling. */
  .stylebar button.icon, .stylebar button.compact { display: inline-flex; align-items: center; justify-content: center; height: 24px; padding: 2px 4px; min-width: 22px; }
  .stylebar input[type="number"] { width: 48px; padding: 2px 4px; }
  .stylebar input.narrow { width: 52px; }
  .stylebar input[type="color"] { width: 28px; height: 22px; padding: 0; border: 1px solid var(--border); background: none; }
  .stylebar button.swatch { width: 18px; min-width: 18px; height: 18px; padding: 0; border-radius: 50%; border: 1px solid var(--border); }
  .stylebar select { padding: 2px 4px; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; width: 92px; flex: none; }
  .stylebar label { display: inline-flex; align-items: center; gap: 3px; }
  .scope { padding-right: 4px; }
  .sep { width: 1px; height: 18px; background: var(--border); margin: 0 2px; }
</style>
