<script lang="ts">
  // The default text style of a placeholder: in a layout (with markers for
  // what it overrides of its theme: ↺ back to the theme, ⇪ make it the
  // theme's) or in a theme (the style of a kind of placeholder).
  import type { PlaceholderStyle } from "./api";

  let {
    ph,
    fonts,
    themeSize,
    themeColor,
    inTheme = false,
    onchange,
    onreset,
    onpush,
  }: {
    ph: PlaceholderStyle & { name: string; own?: string[] };
    fonts: string[];
    themeSize?: number | null;
    themeColor?: string | null;
    /** The placeholder belongs to a layout with a theme: show the markers. */
    inTheme?: boolean;
    onchange: (change: PlaceholderStyle) => void;
    onreset?: (prop: string) => void;
    onpush?: (prop: string) => void;
  } = $props();

  const own = (prop: string) => (ph.own ?? []).includes(prop);
</script>

{#snippet mark(prop: string)}
  {#if inTheme}
    {#if own(prop)}
      <span class="mark own" title="Set by this layout">
        <button title="Back to the theme's value" onclick={() => onreset?.(prop)}>↺</button>
        <button title="Make this the theme's value, for all its layouts" onclick={() => onpush?.(prop)}>⇪</button>
      </span>
    {:else}
      <span class="mark" title="From the theme">theme</span>
    {/if}
  {/if}
{/snippet}

<div class="fields">
  <label class:own={inTheme && own("size")}>size
    <input type="number" min="0" max="200" value={ph.size ?? ""} placeholder={String(themeSize ?? 18)}
           onchange={(e) => onchange({ size: +e.currentTarget.value || 0 })} />{@render mark("size")}</label>
  <label class:own={inTheme && own("bold")}><input type="checkbox" checked={ph.bold ?? false} onchange={(e) => onchange({ bold: e.currentTarget.checked })} /> bold{@render mark("bold")}</label>
  <label class:own={inTheme && own("color")}>color
    <input type="color" value={ph.color ?? themeColor ?? "#000000"} onchange={(e) => onchange({ color: e.currentTarget.value })} />
    <button title="The text color of the theme" onclick={() => onchange({ color: "" })}>⌀</button>{@render mark("color")}</label>
  <label class:own={inTheme && own("font")}>font
    <select value={ph.font ?? ""} onchange={(e) => onchange({ font: e.currentTarget.value })}>
      <option value="">theme font</option>
      {#each fonts as f}<option value={f}>{f}</option>{/each}
    </select>{@render mark("font")}</label>
  <label class:own={inTheme && own("align")}>align
    <select value={ph.align ?? "left"} onchange={(e) => onchange({ align: e.currentTarget.value as "left" | "center" | "right" | "justify" })}>
      <option value="left">left</option><option value="center">center</option><option value="right">right</option><option value="justify">justify</option>
    </select>{@render mark("align")}</label>
  <label class:own={inTheme && own("bullets")}><input type="checkbox" checked={ph.bullets ?? false} onchange={(e) => onchange({ bullets: e.currentTarget.checked })} /> bullets{@render mark("bullets")}</label>
</div>

<style>
  .fields { display: flex; flex-direction: column; gap: 4px; }
  /* Sized here: the app's inputs and selects otherwise take the full width. */
  label { display: flex; align-items: center; gap: 6px; min-width: 0; color: var(--muted); }
  label.own { font-weight: 600; color: var(--fg); }
  label input[type="checkbox"] { width: auto; flex: none; margin: 0; }
  label input[type="number"] { width: 64px; flex: none; }
  label input[type="color"] { width: 34px; height: 22px; flex: none; padding: 0; }
  label select { flex: 1 1 auto; min-width: 0; width: auto; }
  label > button { flex: none; padding: 0 6px; }
  .mark { margin-left: auto; flex: none; color: var(--muted); font-size: 10px; text-transform: uppercase; font-weight: 400; }
  .mark.own { display: inline-flex; gap: 2px; }
  .mark button { font-size: 11px; padding: 0 5px; }
</style>
