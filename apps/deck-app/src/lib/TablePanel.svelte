<script lang="ts">
  // The panel of a table: its cells as a grid of fields, rows and
  // columns added or removed around the cell that has the focus, the
  // header row, and the focused cell's fill.
  import { api, type ElementInfo, type TableEdit } from "./api";

  let { element, onrun }: { element: ElementInfo; onrun: (label: string, f: () => Promise<unknown>) => void } = $props();
  const table = $derived(element.table!);
  const columns = $derived(Math.max(...table.cells.map((r) => r.length), 1));
  /** The cell last focused: where rows and columns are added or removed. */
  let at = $state({ row: 0, column: 0 });
  const row = $derived(Math.min(at.row, table.cells.length - 1));
  const column = $derived(Math.min(at.column, columns - 1));

  function edit(e: TableEdit, label: string) {
    onrun(label, () => api.editTable(element.index, e));
  }
</script>

<h3>Table {table.cells.length} × {columns}</h3>
<div class="grid" style="grid-template-columns: repeat({columns}, minmax(0, 1fr))">
  {#each table.cells as cells, i}
    {#each cells as text, j}
      <textarea rows="1" class:head={table.header && i === 0} class:at={i === row && j === column} value={text}
                style={table.fills[i]?.[j] ? `border-left: 4px solid ${table.fills[i][j]}` : ""}
                onfocus={() => (at = { row: i, column: j })}
                onchange={(e) => edit({ op: "cell", row: i, column: j, text: e.currentTarget.value }, "Cell")}></textarea>
    {/each}
  {/each}
</div>
<div class="row">
  <button title="Insert a row above the focused cell" onclick={() => edit({ op: "add_row", at: row }, "Row added")}>+ row ↑</button>
  <button title="Insert a row below the focused cell" onclick={() => edit({ op: "add_row", at: row + 1 }, "Row added")}>+ row ↓</button>
  <button title="Remove the focused cell's row" disabled={table.cells.length <= 1} onclick={() => edit({ op: "remove_row", at: row }, "Row removed")}>− row</button>
</div>
<div class="row">
  <button title="Insert a column left of the focused cell" onclick={() => edit({ op: "add_column", at: column }, "Column added")}>+ col ←</button>
  <button title="Insert a column right of the focused cell" onclick={() => edit({ op: "add_column", at: column + 1 }, "Column added")}>+ col →</button>
  <button title="Remove the focused cell's column" disabled={columns <= 1} onclick={() => edit({ op: "remove_column", at: column }, "Column removed")}>− col</button>
</div>
<div class="row opts">
  <label><input type="checkbox" checked={table.header} onchange={(e) => edit({ op: "header", on: e.currentTarget.checked }, "Header")} /> header row</label>
  <label title="Fill of the focused cell">cell fill
    <input type="color" value={table.fills[row]?.[column] ?? "#ffffff"} onchange={(e) => edit({ op: "fill", row, column, color: e.currentTarget.value }, "Cell fill")} />
    <button title="No fill" onclick={() => edit({ op: "fill", row, column, color: "" }, "Cell fill removed")}>⌀</button>
  </label>
</div>
<label class="widths" title="Relative widths, one number per column, e.g. 2 1 1; empty for equal columns">column widths
  <input type="text" value={table.columns.join(" ")} placeholder={Array(columns).fill(1).join(" ")}
         onchange={(e) => edit({ op: "columns", widths: e.currentTarget.value.split(/[\s,;]+/).map(Number).filter((n) => n > 0) }, "Column widths")} />
</label>
<p class="muted">One line per paragraph in a cell. Rows share the table's height; resize the table to give them room. The rules' color and width are the stroke of the arrange bar.</p>

<style>
  h3 { margin: 0 0 6px; font-size: 13px; text-transform: uppercase; color: var(--muted); }
  .grid { display: grid; gap: 3px; margin-bottom: 8px; }
  textarea { width: 100%; min-width: 0; padding: 3px 4px; font-size: 12px; resize: vertical; }
  textarea.head { font-weight: 700; }
  textarea.at { border-color: var(--accent); }
  .row { display: flex; gap: 6px; margin-top: 6px; flex-wrap: wrap; align-items: center; }
  .row button { padding: 2px 8px; }
  .opts { font-size: 12px; color: var(--muted); gap: 12px; }
  .opts label { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; }
  .opts input[type="checkbox"] { width: auto; margin: 0; }
  input[type="color"] { width: 28px; height: 22px; padding: 0; border: 1px solid var(--border); background: none; }
  .widths { display: block; margin-top: 8px; font-size: 12px; color: var(--muted); }
  .widths input { width: 100%; }
  .muted { color: var(--muted); font-size: 12px; }
</style>
