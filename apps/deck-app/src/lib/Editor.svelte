<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, watchView, watchThumbnails, presentationKeys, type View, type Rect } from "./api";
  import SlideView from "./SlideView.svelte";

  let view = $state<View | null>(null);
  let thumbnails = $state<string[]>([]);
  let selected = $state<number | null>(null);
  let status = $state("");
  // The "insert slides from another deck" picker.
  let picker = $state<{ path: string; thumbs: string[]; chosen: boolean[] } | null>(null);
  let canvas = $state<HTMLDivElement>();

  onMount(() => {
    const stopView = watchView((v) => {
      view = v;
      if (selected !== null && selected >= v.elements.length) selected = null;
    });
    const stopThumbs = watchThumbnails((t) => (thumbnails = t));
    return () => { stopView(); stopThumbs(); };
  });

  async function run(label: string, f: () => Promise<unknown>) {
    try {
      await f();
      status = label;
    } catch (e) {
      status = `${label}: ${e}`;
    }
  }

  async function openFile() {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Presentations", extensions: ["pptx", "json"] }],
    });
    if (!picked) return;
    // A deck is a folder: opening its deck.json selects the folder.
    const path = picked.endsWith("deck.json") ? picked.slice(0, -"/deck.json".length) : picked;
    await run(`Opened ${path}`, () => api.open(path));
  }

  async function openFolder() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (picked) await run(`Opened ${picked}`, () => api.open(picked));
  }

  async function saveAs() {
    const picked = await saveDialog({ defaultPath: "talk.deck" });
    if (picked) await run(`Saved ${picked}`, () => api.save(picked));
  }

  async function save() {
    if (!view?.dir) return saveAs();
    await run(`Saved ${view.dir}`, () => api.save());
  }

  async function exportPptx() {
    const picked = await saveDialog({ defaultPath: "talk.pptx", filters: [{ name: "PowerPoint", extensions: ["pptx"] }] });
    if (!picked) return;
    await run(`Exported ${picked}`, async () => {
      const warnings = await api.exportPptx(picked);
      if (warnings.length) status = `Exported with ${warnings.length} warnings: ${warnings.join("; ")}`;
    });
  }

  async function pickDeck() {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: "decker deck", extensions: ["json"] }],
      title: "Choose the deck.json of the deck to copy slides from",
    });
    if (!picked) return;
    await run(`Reading ${picked}`, async () => {
      const thumbs = await api.peekDeck(picked);
      picker = { path: picked, thumbs, chosen: thumbs.map(() => false) };
    });
  }

  async function insertPicked() {
    if (!picker) return;
    const indices = picker.chosen.map((c, i) => (c ? i : -1)).filter((i) => i >= 0);
    const p = picker;
    picker = null;
    if (indices.length) await run(`Inserted ${indices.length} slide(s)`, () => api.importSlides(p.path, indices));
  }

  async function exportPdf() {
    const picked = await saveDialog({ defaultPath: "talk.pdf", filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!picked) return;
    await run(`Exported ${picked}`, () => api.exportPdf(picked));
  }

  // Dragging an element on the canvas. Canvas pixels map to slide points
  // through the rendered width.
  let drag: { index: number; startX: number; startY: number; rect: Rect; mode: "move" | "resize" } | null = null;

  // In-place text editing: a textarea laid over the element, in slide
  // coordinates, so it scales with the canvas.
  let editing = $state<{ index: number; text: string; size: number } | null>(null);
  let editor = $state<HTMLTextAreaElement>();

  function startEdit(index: number) {
    if (!view) return;
    const el = view.elements[index];
    if (el.kind === "image") return;
    selected = index;
    // The first run's size, read from the rendered SVG: a reasonable default for the overlay.
    const m = new RegExp(`<g id="e${index}"[\\s\\S]*?font-size="([\\d.]+)"`).exec(view.svg);
    editing = { index, text: el.text, size: m ? parseFloat(m[1]) : 18 };
    queueMicrotask(() => { editor?.focus(); editor?.select(); });
  }

  async function commitEdit() {
    if (!editing) return;
    const { index, text } = editing;
    editing = null;
    if (view && view.elements[index] && text !== view.elements[index].text) {
      await run("Text changed", () => api.setText(index, text));
    }
  }

  function editorKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") { editing = null; }
    else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { commitEdit(); }
  }

  function scale(): number {
    return view && canvas ? canvas.clientWidth / view.width : 1;
  }

  function startDrag(e: PointerEvent, index: number, mode: "move" | "resize") {
    if (!view) return;
    selected = index;
    drag = { index, startX: e.clientX, startY: e.clientY, rect: { ...view.elements[index].rect }, mode };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.stopPropagation();
  }

  function moveDrag(e: PointerEvent) {
    if (!drag || !view) return;
    const dx = (e.clientX - drag.startX) / scale();
    const dy = (e.clientY - drag.startY) / scale();
    const r = drag.rect;
    const next = drag.mode === "move"
      ? { ...r, x: r.x + dx, y: r.y + dy }
      : { ...r, w: Math.max(10, r.w + dx), h: Math.max(10, r.h + dy) };
    view.elements[drag.index].rect = next;
  }

  async function endDrag() {
    if (!drag || !view) return;
    const { index } = drag;
    drag = null;
    await run("Moved", () => api.setRect(index, view!.elements[index].rect));
  }

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "TEXTAREA" || tag === "INPUT") return;
    if (e.key === "Delete" && selected !== null) {
      const i = selected;
      selected = null;
      run("Deleted element", () => api.deleteElement(i));
      return;
    }
    if (e.key === "PageDown" || e.key === "PageUp") {
      api.navigate(e.key === "PageDown" ? "next_slide" : "prev_slide");
      e.preventDefault();
      return;
    }
    if (view?.presenting && presentationKeys(e)) e.preventDefault();
  }

  let sel = $derived(view && selected !== null ? view.elements[selected] : null);
</script>

<svelte:window onkeydown={onKey} />

<div class="editor">
  <header>
    <button onclick={() => run("New deck", api.newDeck)}>New</button>
    <button onclick={openFile}>Open PPTX…</button>
    <button onclick={openFolder}>Open .deck folder…</button>
    <button onclick={save} disabled={!view?.slide_count}>Save</button>
    <button onclick={saveAs} disabled={!view?.slide_count}>Save as…</button>
    <button onclick={exportPptx} disabled={!view?.slide_count}>Export PPTX…</button>
    <button onclick={exportPdf} disabled={!view?.slide_count}>Export PDF…</button>
    <span class="spacer"></span>
    {#if view?.slide_count}
      <button onclick={() => run("Added text", api.addText)}>+ Text</button>
      <button onclick={() => run("Added slide", api.addSlide)}>+ Slide</button>
      <button onclick={() => run("Duplicated slide", api.duplicateSlide)}>Duplicate</button>
      <button onclick={() => run("Deleted slide", api.deleteSlide)}>− Slide</button>
      <button onclick={pickDeck} disabled={!view.dir}>Insert from deck…</button>
      {#if view.presenting}
        <button class="primary" onclick={() => api.stopPresentation()}>Stop (Esc)</button>
      {:else}
        <button class="primary" onclick={() => run("Presenting", api.startPresentation)}>Present</button>
      {/if}
    {/if}
  </header>

  {#if view && view.slide_count}
    <aside class="slides">
      {#each thumbnails as thumb, i}
        <button class="thumb" class:current={i === view.slide} onclick={() => api.goto(i)}>
          <span class="num">{i + 1}</span>
          <SlideView svg={thumb} width={view.width} height={view.height} />
        </button>
      {/each}
    </aside>

    <main>
      <div class="canvas-wrap">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="canvas" bind:this={canvas} style="aspect-ratio: {view.width} / {view.height}"
             onpointermove={moveDrag} onpointerup={endDrag} onclick={() => (selected = null)}>
          <SlideView svg={view.svg} width={view.width} height={view.height} />
          <svg class="overlay" viewBox="0 0 {view.width} {view.height}" preserveAspectRatio="none">
            {#each view.elements as el (el.index)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <rect class="hit" class:selected={el.index === selected}
                    x={el.rect.x} y={el.rect.y} width={el.rect.w} height={el.rect.h}
                    onpointerdown={(e) => startDrag(e, el.index, "move")}
                    ondblclick={(e) => { e.stopPropagation(); startEdit(el.index); }} />
              {#if el.index === selected}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <rect class="handle" x={el.rect.x + el.rect.w - 6} y={el.rect.y + el.rect.h - 6} width="12" height="12"
                      onpointerdown={(e) => startDrag(e, el.index, "resize")} />
              {/if}
            {/each}
          </svg>
          {#if editing && view.elements[editing.index]}
            {@const r = view.elements[editing.index].rect}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <textarea class="inline-editor" bind:this={editor} bind:value={editing.text}
                      style="left: {(r.x / view.width) * 100}%; top: {(r.y / view.height) * 100}%; width: {(r.w / view.width) * 100}%; height: {(r.h / view.height) * 100}%; font-size: {editing.size * scale()}px"
                      onkeydown={editorKey} onblur={commitEdit} onclick={(e) => e.stopPropagation()} onpointerdown={(e) => e.stopPropagation()}
                      spellcheck="false"></textarea>
          {/if}
        </div>
      </div>
      <div class="steps">
        Slide {view.slide + 1} / {view.slide_count}
        {#if view.last_step > 0}
          · step {view.step} / {view.last_step}
          <button onclick={() => api.goto(view!.slide, Math.max(0, view!.step - 1))}>◀</button>
          <button onclick={() => api.goto(view!.slide, view!.step + 1)}>▶</button>
        {/if}
      </div>
    </main>

    <aside class="props">
      {#if sel}
        <h3>{sel.kind} #{sel.index}</h3>
        <label>x <input type="number" value={Math.round(sel.rect.x)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, x: +e.currentTarget.value })} /></label>
        <label>y <input type="number" value={Math.round(sel.rect.y)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, y: +e.currentTarget.value })} /></label>
        <label>w <input type="number" value={Math.round(sel.rect.w)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, w: +e.currentTarget.value })} /></label>
        <label>h <input type="number" value={Math.round(sel.rect.h)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, h: +e.currentTarget.value })} /></label>
        {#if sel.kind !== "image"}
          <label>{sel.kind === "math" ? "formula" : "text"}
            <textarea rows="6" value={sel.text} onchange={(e) => api.setText(sel!.index, e.currentTarget.value)}></textarea>
          </label>
        {/if}
        {#if sel.last_step > 0}<p class="muted">{sel.last_step} build step(s)</p>{/if}
      {:else}
        <p class="muted">Click an element to select it, double-click to edit its text in place (Ctrl+Enter applies, Escape cancels). Drag to move, corner handle to resize, Delete to remove.</p>
      {/if}
      <h3>Section</h3>
      <label>starts section
        <input type="text" value={view.section ?? ""} placeholder={view.in_section ?? "none"}
               onchange={(e) => run("Section set", () => api.setSection(e.currentTarget.value || null, view!.subsection))} />
      </label>
      <label>starts subsection
        <input type="text" value={view.subsection ?? ""} placeholder={view.in_subsection ?? "none"}
               onchange={(e) => run("Subsection set", () => api.setSection(view!.section, e.currentTarget.value || null))} />
      </label>
      <p class="muted">Sections become PDF bookmarks and PowerPoint sections.</p>
      <h3>Layout</h3>
      {#if view.layouts.length}
        <select value={view.layout ?? ""} onchange={(e) => run("Layout set", () => api.setLayout(e.currentTarget.value || null))}>
          <option value="">none</option>
          {#each view.layouts as name}<option value={name}>{name}</option>{/each}
        </select>
        <p class="muted">"+ Slide" starts from this layout's placeholders.</p>
      {:else}
        <p class="muted">No layouts in this deck. Importing a PPTX brings its masters along.</p>
      {/if}
      <h3>Notes</h3>
      <textarea rows="8" value={view.notes} onchange={(e) => api.setNotes(e.currentTarget.value)}></textarea>
      {#if view.warnings.length}
        <h3>Import warnings</h3>
        <ul class="muted">{#each view.warnings as w}<li>{w}</li>{/each}</ul>
      {/if}
    </aside>
  {:else}
    <div class="empty">
      <p>Open a PowerPoint file or a .deck folder, or start a new deck.</p>
    </div>
  {/if}

  <footer>{status}</footer>

  {#if picker}
    <div class="modal">
      <div class="dialog">
        <h3>Insert slides from {picker.path}</h3>
        <div class="grid">
          {#each picker.thumbs as thumb, i}
            <label class="pick" class:on={picker.chosen[i]}>
              <input type="checkbox" bind:checked={picker.chosen[i]} />
              <span class="num">{i + 1}</span>
              <SlideView svg={thumb} width={view?.width ?? 960} height={view?.height ?? 540} />
            </label>
          {/each}
        </div>
        <div class="actions">
          <button onclick={() => (picker = null)}>Cancel</button>
          <button onclick={() => picker && (picker.chosen = picker.chosen.map(() => true))}>All</button>
          <button class="primary" onclick={insertPicked}>Insert after current slide</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .editor {
    display: grid;
    grid-template-columns: 180px 1fr 280px;
    grid-template-rows: auto 1fr auto;
    grid-template-areas: "head head head" "slides main props" "foot foot foot";
    height: 100%;
  }
  header { grid-area: head; display: flex; gap: 6px; padding: 8px; border-bottom: 1px solid var(--border); align-items: center; }
  .spacer { flex: 1; }
  .slides { grid-area: slides; overflow-y: auto; padding: 8px; border-right: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  .thumb { padding: 2px; position: relative; background: none; }
  .thumb.current { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .thumb .num { position: absolute; left: -2px; top: -2px; background: var(--panel); font-size: 11px; padding: 1px 4px; border-radius: 4px; z-index: 1; }
  main { grid-area: main; display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .canvas-wrap { flex: 1; display: flex; align-items: center; justify-content: center; padding: 16px; min-height: 0; }
  .canvas { position: relative; width: 100%; max-height: 100%; box-shadow: 0 4px 24px rgba(0,0,0,0.5); }
  .overlay { position: absolute; inset: 0; width: 100%; height: 100%; }
  .hit { fill: transparent; stroke: transparent; stroke-width: 1; cursor: move; }
  .hit:hover { stroke: var(--accent); stroke-dasharray: 4 2; }
  .hit.selected { stroke: var(--accent); stroke-width: 2; }
  .handle { fill: var(--accent); cursor: nwse-resize; }
  .inline-editor {
    position: absolute; box-sizing: border-box; margin: 0; padding: 4px; resize: none; overflow: hidden;
    background: rgba(255, 255, 255, 0.92); color: black; border: 2px solid var(--accent); border-radius: 0;
    font-family: sans-serif; line-height: 1.2; outline: none;
  }
  .steps { padding: 6px 16px; color: var(--muted); display: flex; gap: 8px; align-items: center; }
  .props { grid-area: props; overflow-y: auto; padding: 12px; border-left: 1px solid var(--border); }
  .props h3 { margin: 12px 0 6px; font-size: 13px; color: var(--muted); text-transform: uppercase; }
  .props label { display: block; margin-bottom: 6px; color: var(--muted); }
  .muted { color: var(--muted); }
  .empty { grid-area: main; display: flex; align-items: center; justify-content: center; color: var(--muted); grid-column: 1 / -1; }
  select { width: 100%; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; padding: 6px; }
  .modal { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; }
  .dialog { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 16px; width: min(900px, 90vw); max-height: 85vh; display: flex; flex-direction: column; gap: 12px; }
  .dialog h3 { margin: 0; font-size: 14px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px; overflow-y: auto; }
  .pick { position: relative; display: block; border: 2px solid var(--border); border-radius: 6px; padding: 2px; cursor: pointer; }
  .pick.on { border-color: var(--accent); }
  .pick input { position: absolute; left: 6px; top: 6px; z-index: 1; width: auto; }
  .pick .num { position: absolute; right: 6px; top: 4px; font-size: 11px; background: var(--panel); padding: 1px 4px; border-radius: 4px; z-index: 1; }
  .actions { display: flex; gap: 8px; justify-content: flex-end; }
  footer { grid-area: foot; padding: 4px 12px; border-top: 1px solid var(--border); color: var(--muted); font-size: 12px; min-height: 24px; }
</style>
