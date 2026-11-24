<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, watchView, watchThumbnails, presentationKeys, type View, type Rect, type Crop, type Mask, type TextLayout } from "./api";
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
      if (editing && (editing.index >= v.elements.length || v.slide !== editingSlide)) editing = null;
      editingSlide = v.slide;
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

  // The outline in the slide stack: headers above the slides that start a
  // section or subsection, renamed in place; a header being renamed.
  let renaming = $state<{ slide: number; level: "section" | "subsection" } | null>(null);
  let renameInput = $state<HTMLInputElement>();

  function startHeading(slide: number, level: "section" | "subsection") {
    if (!view) return;
    const st = view.starts[slide];
    const name = level === "section" ? "Section" : "Subsection";
    const section = level === "section" ? name : st.section;
    const subsection = level === "subsection" ? name : st.subsection;
    run(`${name} added`, async () => {
      await api.setSectionOf(slide, section, subsection);
      renaming = { slide, level };
      queueMicrotask(() => { renameInput?.focus(); renameInput?.select(); });
    });
  }

  function renameHeading(slide: number, level: "section" | "subsection", value: string) {
    if (!view) return;
    const st = view.starts[slide];
    const v = value.trim() || null;
    renaming = null;
    run(v ? "Renamed" : "Heading removed", () =>
      api.setSectionOf(slide, level === "section" ? v : st.section, level === "subsection" ? v : st.subsection));
  }

  // Dragging an element on the canvas. Canvas pixels map to slide points
  // through the rendered width.
  let drag: { index: number; startX: number; startY: number; rect: Rect; mode: "move" | "resize" } | null = null;

  // In-place text editing with a caret drawn in the overlay. The caret is
  // a flat character index into the element's text; Rust applies edits and
  // returns the layout of every character, so what you see is the typeset
  // text itself. Without the Typst feature the layout is null and the side
  // panel's text field remains the way to edit.
  let editing = $state<{ index: number; caret: number; anchor: number; layout: TextLayout } | null>(null);
  let caretGoalX: number | null = null;
  // Key actions run one after another, each seeing the state left by the
  // previous one, so fast typing never drops or reorders characters.
  let queue: Promise<void> = Promise.resolve();
  function enqueue(action: () => void | Promise<void>) {
    queue = queue.then(action).catch((e) => { status = `Edit failed: ${e}`; });
  }
  let editingSlide = -1;

  async function startEdit(index: number, at?: { x: number; y: number }) {
    if (!view) return;
    const el = view.elements[index];
    if (el.kind === "image" || el.kind === "math") return;
    selected = index;
    const layout = await api.textLayout(index);
    if (!layout) { status = "Text editing in place needs the Typst feature"; return; }
    const caret = at ? caretAt(layout, at.x, at.y) : layout.chars.length - 1;
    editing = { index, caret, anchor: caret, layout };
  }

  async function refreshLayout() {
    if (!editing) return;
    const layout = await api.textLayout(editing.index);
    if (!editing || !layout) { editing = null; return; }
    editing.layout = layout;
    const last = layout.chars.length - 1;
    editing.caret = Math.min(editing.caret, last);
    editing.anchor = Math.min(editing.anchor, last);
  }

  // The character index nearest a point in slide coordinates: the line
  // whose box contains y (or the closest), then the nearest edge in x.
  function caretAt(layout: TextLayout, x: number, y: number): number {
    const chars = layout.chars;
    if (chars.length <= 1) return 0;
    let bestLine = Infinity, lineTop = 0, lineBottom = 0;
    for (const c of chars) {
      const d = y < c.top ? c.top - y : y > c.bottom ? y - c.bottom : 0;
      if (d < bestLine) { bestLine = d; lineTop = c.top; lineBottom = c.bottom; }
    }
    let best = chars.length - 1, bestD = Infinity;
    chars.forEach((c, i) => {
      if (Math.abs(c.top - lineTop) > 0.5 && Math.abs(c.bottom - lineBottom) > 0.5) return;
      // The nearest character edge: its left edge gives this index, its right edge the next.
      const candidates: [number, number][] = [[i, Math.abs(x - c.x)], [Math.min(i + 1, chars.length - 1), Math.abs(x - c.right)]];
      for (const [idx, d] of candidates) if (d < bestD) { bestD = d; best = idx; }
    });
    return best;
  }

  function lineOf(layout: TextLayout, i: number) {
    const c = layout.chars[Math.min(i, layout.chars.length - 1)];
    return { top: c.top, bottom: c.bottom };
  }

  function moveVertical(dir: -1 | 1) {
    if (!editing) return editing!.caret;
    const { layout, caret } = editing;
    const cur = lineOf(layout, caret);
    const x = caretGoalX ?? layout.chars[caret].x;
    caretGoalX = x;
    // The nearest line strictly above or below the current one.
    let target: { top: number; bottom: number } | null = null;
    for (const c of layout.chars) {
      const above = c.bottom <= cur.top + 0.5, below = c.top >= cur.bottom - 0.5;
      if ((dir < 0 && above) || (dir > 0 && below)) {
        if (!target || (dir < 0 ? c.top > target.top : c.top < target.top)) target = { top: c.top, bottom: c.bottom };
      }
    }
    if (!target) return dir < 0 ? 0 : layout.chars.length - 1;
    return caretAt(layout, x, (target.top + target.bottom) / 2);
  }

  async function applyEdit(from: number, to: number, insert: string) {
    if (!editing) return;
    const caret = await api.textEdit(editing.index, from, to, insert);
    if (editing) { editing.caret = caret; editing.anchor = caret; }
    await refreshLayout();
  }

  /// A key: queued, so that positions are read when the action runs.
  function editorKey(e: KeyboardEvent): boolean {
    if (!editing) return false;
    const handled = keyAction(e.key, e.shiftKey, e.ctrlKey || e.metaKey, e.altKey);
    if (handled) enqueue(handled);
    return handled !== null;
  }

  function keyAction(key: string, shift: boolean, ctrl: boolean, alt: boolean): (() => void | Promise<void>) | null {
    const withState = (f: (st: { caret: number; anchor: number; layout: TextLayout; lo: number; hi: number; last: number }) => void | Promise<void>) => () => {
      if (!editing) return;
      const { caret, anchor, layout } = editing;
      return f({ caret, anchor, layout, lo: Math.min(caret, anchor), hi: Math.max(caret, anchor), last: layout.chars.length - 1 });
    };
    const setCaret = (i: number, keepAnchor: boolean) => {
      if (!editing) return;
      editing.caret = Math.max(0, Math.min(editing.layout.chars.length - 1, i));
      if (!keepAnchor) editing.anchor = editing.caret;
    };
    if (key !== "ArrowUp" && key !== "ArrowDown") caretGoalX = null;
    switch (key) {
      case "Escape": return () => { editing = null; };
      case "ArrowLeft": return withState(({ caret, lo, hi }) => setCaret(lo !== hi && !shift ? lo : caret - 1, shift));
      case "ArrowRight": return withState(({ caret, lo, hi }) => setCaret(lo !== hi && !shift ? hi : caret + 1, shift));
      case "ArrowUp": return () => setCaret(moveVertical(-1), shift);
      case "ArrowDown": return () => setCaret(moveVertical(1), shift);
      case "Home": return withState(({ caret, layout }) => { const l = lineOf(layout, caret); let i = caret; while (i > 0 && Math.abs(layout.chars[i - 1].top - l.top) < 0.5) i--; setCaret(i, shift); });
      case "End": return withState(({ caret, layout, last }) => { const l = lineOf(layout, caret); let i = caret; while (i < last && Math.abs(layout.chars[i + 1].top - l.top) < 0.5 && layout.text[i] !== "\n") i++; setCaret(i, shift); });
      case "Backspace": return withState(({ caret, lo, hi }) => (lo !== hi ? applyEdit(lo, hi, "") : caret > 0 ? applyEdit(caret - 1, caret, "") : undefined));
      case "Delete": return withState(({ caret, lo, hi, last }) => (lo !== hi ? applyEdit(lo, hi, "") : caret < last ? applyEdit(caret, caret + 1, "") : undefined));
      case "Enter": return withState(({ lo, hi }) => applyEdit(lo, hi, "\n"));
    }
    if (key === "a" && ctrl) return withState(({ last }) => { if (editing) { editing.anchor = 0; editing.caret = last; } });
    if (key.length === 1 && !ctrl && !alt) return withState(({ lo, hi }) => applyEdit(lo, hi, key));
    return null;
  }

  // Slide coordinates of a pointer event on the canvas.
  function slidePoint(e: PointerEvent | MouseEvent): { x: number; y: number } {
    const r = canvas!.getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * view!.width, y: ((e.clientY - r.top) / r.height) * view!.height };
  }

  function editorPointerDown(e: PointerEvent) {
    if (!editing) return;
    const p = slidePoint(e);
    const shift = e.shiftKey;
    enqueue(() => {
      if (!editing) return;
      const i = caretAt(editing.layout, p.x, p.y);
      editing.caret = i;
      if (!shift) editing.anchor = i;
      caretGoalX = null;
    });
    e.stopPropagation();
  }

  // Selection rectangles, one per line.
  function selectionRects(ed: { caret: number; anchor: number; layout: TextLayout }): { x: number; y: number; w: number; h: number }[] {
    const lo = Math.min(ed.caret, ed.anchor), hi = Math.max(ed.caret, ed.anchor);
    const out: { x: number; y: number; w: number; h: number }[] = [];
    for (let i = lo; i < hi; i++) {
      const c = ed.layout.chars[i];
      const cur = out[out.length - 1];
      if (cur && Math.abs(cur.y - c.top) < 0.5) { cur.w = Math.max(cur.w, c.right - cur.x); }
      else out.push({ x: c.x, y: c.top, w: c.right - c.x, h: c.bottom - c.top });
    }
    return out;
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
    if (tag === "TEXTAREA" || tag === "INPUT" || tag === "SELECT") return;
    if (editing) {
      if (editorKey(e)) e.preventDefault();
      return;
    }
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

  // Image crop and mask edits, from percentages in the panel.
  function setCrop(edge: keyof Crop, percent: number) {
    if (!sel || !sel.crop || !sel.mask) return;
    const crop = { ...sel.crop, [edge]: Math.max(0, Math.min(95, percent)) / 100 };
    run("Crop set", () => api.setImage(sel!.index, crop, sel!.mask!));
  }
  function setMask(kind: string) {
    if (!sel || !sel.crop) return;
    const mask: Mask = kind === "ellipse" ? { kind: "ellipse" } : kind === "rounded_rectangle" ? { kind: "rounded_rectangle", radius: 0.15 } : { kind: "rectangle" };
    run("Mask set", () => api.setImage(sel!.index, sel!.crop!, mask));
  }
  function setRadius(percent: number) {
    if (!sel || !sel.crop) return;
    run("Radius set", () => api.setImage(sel!.index, sel!.crop!, { kind: "rounded_rectangle", radius: Math.max(0, Math.min(50, percent)) / 100 }));
  }
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
        {@const st = view.starts[i] ?? { section: null, subsection: null }}
        {#each ["section", "subsection"] as level}
          {@const name = level === "section" ? st.section : st.subsection}
          {#if name !== null}
            <div class="heading {level}">
              {#if renaming && renaming.slide === i && renaming.level === level}
                <input bind:this={renameInput} value={name}
                       onblur={(e) => renameHeading(i, level as "section" | "subsection", e.currentTarget.value)}
                       onkeydown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); if (e.key === "Escape") { renaming = null; } e.stopPropagation(); }} />
              {:else}
                <button class="heading-name" title="Click to rename, clear to remove" onclick={() => { renaming = { slide: i, level: level as "section" | "subsection" }; queueMicrotask(() => { renameInput?.focus(); renameInput?.select(); }); }}>{name}</button>
              {/if}
            </div>
          {/if}
        {/each}
        <div class="stack-item">
          <button class="thumb" class:current={i === view.slide} onclick={() => api.goto(i)}>
            <span class="num">{i + 1}</span>
            <SlideView svg={thumb} width={view.width} height={view.height} />
          </button>
          <div class="adders">
            {#if st.section === null}<button title="Start a section at this slide" onclick={() => startHeading(i, "section")}>+ section</button>{/if}
            {#if st.subsection === null}<button title="Start a subsection at this slide" onclick={() => startHeading(i, "subsection")}>+ sub</button>{/if}
          </div>
        </div>
      {/each}
    </aside>

    <main>
      <div class="canvas-wrap">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="canvas" bind:this={canvas} style="aspect-ratio: {view.width} / {view.height}"
             onpointermove={moveDrag} onpointerup={endDrag} onclick={() => { selected = null; editing = null; }}>
          <SlideView svg={view.svg} width={view.width} height={view.height} />
          <svg class="overlay" viewBox="0 0 {view.width} {view.height}" preserveAspectRatio="none">
            {#each view.elements as el (el.index)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <rect class="hit" class:selected={el.index === selected}
                    x={el.rect.x} y={el.rect.y} width={el.rect.w} height={el.rect.h}
                    onpointerdown={(e) => startDrag(e, el.index, "move")}
                    ondblclick={(e) => { e.stopPropagation(); startEdit(el.index, slidePoint(e)); }} />
              {#if el.index === selected}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <rect class="handle" x={el.rect.x + el.rect.w - 6} y={el.rect.y + el.rect.h - 6} width="12" height="12"
                      onpointerdown={(e) => startDrag(e, el.index, "resize")} />
              {/if}
            {/each}
          </svg>
          {#if editing && view.elements[editing.index]}
            {@const r = view.elements[editing.index].rect}
            {@const c = editing.layout.chars[editing.caret]}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <svg class="overlay text-editing" viewBox="0 0 {view.width} {view.height}" preserveAspectRatio="none"
                 onpointerdown={editorPointerDown} onclick={(e) => e.stopPropagation()}>
              <rect class="edit-frame" x={r.x} y={r.y} width={r.w} height={r.h} />
              {#each selectionRects(editing) as s}
                <rect class="selection" x={s.x} y={s.y} width={s.w} height={s.h} />
              {/each}
              <line class="caret" x1={c.x} y1={c.top} x2={c.x} y2={c.bottom} />
            </svg>
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
        {#if sel.kind === "image" && sel.crop && sel.mask}
          <h3>Crop to shape</h3>
          <select value={sel.mask.kind} onchange={(e) => setMask(e.currentTarget.value)}>
            <option value="rectangle">rectangle</option>
            <option value="ellipse">ellipse</option>
            <option value="rounded_rectangle">rounded rectangle</option>
          </select>
          {#if sel.mask.kind === "rounded_rectangle"}
            <label>corner radius % <input type="number" min="0" max="50" value={Math.round(sel.mask.radius * 100)} onchange={(e) => setRadius(+e.currentTarget.value)} /></label>
          {/if}
          <h3>Crop edges %</h3>
          <div class="crop-grid">
            <label>left <input type="number" min="0" max="95" value={Math.round(sel.crop.left * 100)} onchange={(e) => setCrop("left", +e.currentTarget.value)} /></label>
            <label>top <input type="number" min="0" max="95" value={Math.round(sel.crop.top * 100)} onchange={(e) => setCrop("top", +e.currentTarget.value)} /></label>
            <label>right <input type="number" min="0" max="95" value={Math.round(sel.crop.right * 100)} onchange={(e) => setCrop("right", +e.currentTarget.value)} /></label>
            <label>bottom <input type="number" min="0" max="95" value={Math.round(sel.crop.bottom * 100)} onchange={(e) => setCrop("bottom", +e.currentTarget.value)} /></label>
          </div>
          <p class="muted">The kept part of the image fills the box.</p>
        {:else if sel.kind !== "image"}
          <label>{sel.kind === "math" ? "formula" : "text"}
            <textarea rows="6" value={sel.text} onchange={(e) => api.setText(sel!.index, e.currentTarget.value)}></textarea>
          </label>
        {/if}
        {#if sel.last_step > 0}<p class="muted">{sel.last_step} build step(s)</p>{/if}
      {:else}
        <p class="muted">Click an element to select it, double-click its text to edit it in place (arrows, Shift for a selection, Enter for a paragraph, Escape to finish). Drag to move, corner handle to resize, Delete to remove.</p>
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
  .stack-item { position: relative; }
  .stack-item .adders { display: none; position: absolute; right: 4px; bottom: 4px; gap: 3px; }
  .stack-item:hover .adders { display: flex; }
  .adders button { font-size: 10px; padding: 1px 5px; background: var(--panel); }
  .heading { display: flex; align-items: center; gap: 6px; padding: 6px 2px 0; }
  .heading.section { border-top: 1px solid var(--border); margin-top: 4px; }
  .heading.subsection { padding-left: 12px; }
  .heading-name { background: none; border: none; padding: 0; color: var(--fg); font-weight: 600; font-size: 12px; text-align: left; cursor: text; }
  .heading.subsection .heading-name { font-weight: 500; color: var(--muted); }
  .heading input { font-size: 12px; padding: 2px 4px; }
  .thumb { padding: 2px; position: relative; background: none; width: 100%; }
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
  .text-editing { cursor: text; }
  .edit-frame { fill: none; stroke: var(--accent); stroke-width: 1.5; stroke-dasharray: 4 3; }
  .selection { fill: rgba(79, 140, 255, 0.35); }
  .caret { stroke: black; stroke-width: 1.5; animation: blink 1s steps(2, start) infinite; }
  @keyframes blink { to { visibility: hidden; } }
  .steps { padding: 6px 16px; color: var(--muted); display: flex; gap: 8px; align-items: center; }
  .props { grid-area: props; overflow-y: auto; padding: 12px; border-left: 1px solid var(--border); }
  .props h3 { margin: 12px 0 6px; font-size: 13px; color: var(--muted); text-transform: uppercase; }
  .props label { display: block; margin-bottom: 6px; color: var(--muted); }
  .muted { color: var(--muted); }
  .crop-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 8px; }
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
