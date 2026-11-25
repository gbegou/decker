<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, watchView, watchThumbnails, presentationKeys, type View, type Rect, type Crop, type Mask, type TextLayout } from "./api";
  import { SlideView } from "$shared";
  import StyleBar from "./StyleBar.svelte";
  import ArrangeBar from "./ArrangeBar.svelte";
  import Help from "./Help.svelte";
  import Steps from "./Steps.svelte";

  let help = $state(false);

  let view = $state<View | null>(null);
  let thumbnails = $state<string[]>([]);
  let selected = $state<number | null>(null);
  // Further selected elements (Shift+click), for grouping.
  let also = $state<number[]>([]);
  const selection = $derived(selected === null ? [] : [selected, ...also.filter((i) => i !== selected)]);
  let status = $state("");
  // The "insert slides from another deck" picker.
  let picker = $state<{ path: string; thumbs: string[]; chosen: boolean[] } | null>(null);
  let canvas = $state<HTMLDivElement>();

  onMount(() => {
    const stopView = watchView((v) => {
      view = v;
      if (selected !== null && selected >= v.elements.length) selected = null;
      also = also.filter((i) => i < v.elements.length);
      if (editing && (editing.index >= v.elements.length || v.slide !== editingSlide || v.editing_layout !== editingLayout)) editing = null;
      editingSlide = v.slide;
      editingLayout = v.editing_layout;
    });
    const stopThumbs = watchThumbnails((t) => (thumbnails = t));
    // Files dragged from the desktop: the OS hands their paths to the window.
    let stopDrop = () => {};
    getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") dropHover = true;
      else if (p.type === "leave") dropHover = false;
      else if (p.type === "drop") {
        dropHover = false;
        const ratio = window.devicePixelRatio || 1;
        const at = pointOnSlide(p.position.x / ratio, p.position.y / ratio);
        for (const path of p.paths.filter((f) => IMAGE_FILE.test(f))) {
          enqueue(() => run("Image added", () => api.addImageFile(path, at?.x, at?.y)).then(selectAdded));
        }
      }
    }).then((stop) => (stopDrop = stop));
    return () => { stopView(); stopThumbs(); stopDrop(); };
  });

  const IMAGE_FILE = /\.(png|jpe?g|gif|webp|bmp|svg|mp4|webm|mov|m4v|ogv)$/i;

  /** "+ Video…": a video or GIF file picked from disk. */
  async function addMedia() {
    const picked = await openDialog({ multiple: false, filters: [{ name: "Video or GIF", extensions: ["mp4", "webm", "mov", "m4v", "ogv", "gif"] }] });
    if (typeof picked === "string") enqueue(() => run("Media added", () => api.addImageFile(picked)).then(selectAdded));
  }
  let dropHover = $state(false);

  /** Slide coordinates of a point in CSS pixels of the window, when it is over the canvas. */
  function pointOnSlide(cx: number, cy: number): { x: number; y: number } | undefined {
    if (!canvas || !view) return undefined;
    const r = canvas.getBoundingClientRect();
    if (cx < r.left || cx > r.right || cy < r.top || cy > r.bottom) return undefined;
    return { x: ((cx - r.left) / r.width) * view.width, y: ((cy - r.top) / r.height) * view.height };
  }

  /** Selects the element an add command returned the index of. */
  function selectAdded(index: unknown) {
    if (typeof index === "number") { editing = null; selected = index; also = []; }
  }

  /** Selects the elements a paste or an add command returned. */
  function selectAll(indices: number[]) {
    if (!indices.length) return;
    editing = null;
    selected = indices[0];
    also = indices.slice(1);
  }

  /** Ctrl+V outside inputs: copied elements, a clipboard image, or text into the box being edited. */
  function pasteClipboard() {
    const ed = editing;
    enqueue(() =>
      run("Pasted", api.paste).then((r) => {
        if (!r) return;
        if (r.added.length) selectAll(r.added);
        else if (r.text && ed) return applyEdit(Math.min(ed.caret, ed.anchor), Math.max(ed.caret, ed.anchor), r.text);
        else status = "Nothing to paste";
      }),
    );
  }

  // ----- Python macros
  let scripts = $state<{ scripts: string[]; python: string | null } | null>(null);
  let scriptOutput = $state<string>("");
  let scriptDir = $state<string>("");
  $effect(() => {
    // Reload the list when the deck folder changes (open, save as).
    const d = view?.dir ?? "";
    if (d !== scriptDir) { scriptDir = d; api.listScripts().then((s) => (scripts = s)).catch(() => (scripts = null)); }
  });
  function runMacro(name: string) {
    const sel = selected === null ? [] : [selected, ...also];
    enqueue(() =>
      run(`Ran ${name.split(/[\\/]/).pop()}`, () => api.runScript(name, sel)).then((r) => {
        if (!r) return;
        scriptOutput = (r.stdout + (r.stderr ? "\n" + r.stderr : "")).trim() || (r.ok ? "(no output)" : "failed");
        if (!r.ok) status = `${name}: failed, see the output`;
        else if (!r.changed) status = `${name}: ran, deck unchanged`;
        if (r.changed) { selected = null; also = []; editing = null; }
        api.listScripts().then((s) => (scripts = s)).catch(() => {});
      }),
    );
  }
  async function runScriptFile() {
    const picked = await openDialog({ multiple: false, filters: [{ name: "Python script", extensions: ["py"] }] });
    if (typeof picked === "string") runMacro(picked);
  }

  /** Ctrl+Z / Ctrl+Y: the whole deck steps back or forward; the selection is dropped. */
  function undoRedo(redo: boolean) {
    enqueue(() =>
      run(redo ? "Redone" : "Undone", redo ? api.redo : api.undo).then(() => {
        selected = null; also = []; editing = null;
      }),
    );
  }

  /** Ctrl+C / Ctrl+X on the selection (not while editing text). */
  function copySelection(cut: boolean) {
    if (selected === null) return;
    const indices = [selected, ...also];
    run(cut ? "Cut" : "Copied", async () => {
      await api.copyElements(indices);
      if (cut) {
        selected = null; also = []; editing = null;
        for (const i of [...indices].sort((a, b) => b - a)) await api.deleteElement(i);
      }
    });
  }

  async function run<T>(label: string, f: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await f();
      status = label;
      return result;
    } catch (e) {
      status = `${label}: ${e}`;
      return undefined;
    }
  }

  async function openFile() {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Presentations", extensions: ["pptx", "deckz", "json"] }],
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
    // A folder name (talk.deck) keeps the deck as a folder; a .deckz name packs it into one file.
    const picked = await saveDialog({
      defaultPath: "talk.deck",
      filters: [{ name: "Deck folder", extensions: ["deck"] }, { name: "Single-file deck", extensions: ["deckz"] }],
    });
    if (picked) await run("Saved", async () => { status = `Saved ${await api.save(picked)}`; });
  }

  async function save() {
    if (!view?.dir) return saveAs();
    let where = view.dir;
    await run("Saved", async () => { where = await api.save(); });
    status = `Saved ${where}`;
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

  async function saveLayoutAs() {
    const picked = await saveDialog({ defaultPath: `${view?.layout ?? "layout"}.deck_layout`, title: "Save the layout as a .deck_layout folder" });
    if (!picked) return;
    await run(`Layout saved to ${picked}`, () => api.saveLayout(picked));
  }

  async function loadLayoutFrom() {
    const picked = await openDialog({ multiple: false, filters: [{ name: "decker layout", extensions: ["json"] }], title: "Choose the layout.json of a .deck_layout folder" });
    if (!picked) return;
    await run(`Layout loaded from ${picked}`, () => api.loadLayout(picked));
  }

  // "Export PDF…" first asks how: one page per slide, or one per build step.
  let pdfDialog = $state(false);
  let pdfSteps = $state(false);
  async function exportPdf() {
    pdfDialog = false;
    const picked = await saveDialog({ defaultPath: "talk.pdf", filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!picked) return;
    await run(`Exported ${picked}`, () => api.exportPdf(picked, pdfSteps));
  }

  // Reordering slides by dragging thumbnails: the drop target is the gap
  // before or after a thumbnail, from the pointer's vertical position.
  let dragging = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  function dragStart(e: DragEvent, i: number) {
    dragging = i;
    e.dataTransfer?.setData("text/plain", String(i));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
  function dragOver(e: DragEvent, i: number) {
    if (dragging === null) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropAt = e.clientY < r.top + r.height / 2 ? i : i + 1;
  }
  function dragEnd() {
    if (dragging !== null && dropAt !== null) {
      const from = dragging;
      // Dropping after the dragged slide: the index shifts once it is removed.
      const to = dropAt > from ? dropAt - 1 : dropAt;
      if (to !== from) run(`Moved slide ${from + 1} to ${to + 1}`, () => api.moveSlide(from, to));
    }
    dragging = null;
    dropAt = null;
  }

  async function groupSelection() {
    const idx = selection;
    if (idx.length < 2) return;
    await run("Grouped", async () => {
      const at = await api.groupElements(idx);
      also = [];
      selected = at;
    });
  }

  async function ungroupSelection() {
    if (!sel || sel.kind !== "group") return;
    const at = sel.index;
    await run("Ungrouped", async () => {
      const n = await api.ungroupElement(at);
      selected = at;
      also = Array.from({ length: n - 1 }, (_, k) => at + 1 + k);
    });
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
  // A drag moves every selected element together (or resizes the one
  // whose handle is held); `rects` are their boxes when it started.
  let drag: { index: number; startX: number; startY: number; rects: Map<number, Rect>; mode: "move" | "resize" } | null = null;
  /** Guide lines shown while a drag snaps: slide coordinates. */
  let guides = $state<{ v: number[]; h: number[] }>({ v: [], h: [] });

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
  let editingLayout: string | null = null;

  async function startEdit(index: number, at?: { x: number; y: number }) {
    if (!view) return;
    const el = view.elements[index];
    if (el.kind === "image" || el.kind === "math" || el.kind === "placeholder" || el.kind === "outline") return;
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

  // Mouse selection: press sets the caret (Shift extends), dragging
  // extends the selection, a double click takes the word and a triple
  // click the paragraph.
  let selecting = false;
  function editorPointerDown(e: PointerEvent) {
    if (!editing) return;
    const p = slidePoint(e);
    const shift = e.shiftKey;
    selecting = true;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    enqueue(() => {
      if (!editing) return;
      const i = caretAt(editing.layout, p.x, p.y);
      editing.caret = i;
      if (!shift) editing.anchor = i;
      caretGoalX = null;
    });
    e.stopPropagation();
  }
  function editorPointerMove(e: PointerEvent) {
    if (!editing || !selecting) return;
    const p = slidePoint(e);
    enqueue(() => {
      if (editing && selecting) editing.caret = caretAt(editing.layout, p.x, p.y);
    });
  }
  function editorPointerUp() {
    selecting = false;
  }
  /** Selects the word (2 clicks) or the paragraph (3) at the click. */
  function editorMultiClick(e: MouseEvent) {
    e.stopPropagation();
    if (!editing || e.detail < 2) return;
    const p = slidePoint(e);
    const clicks = e.detail;
    enqueue(() => {
      if (!editing) return;
      const text = editing.layout.text;
      const i = Math.min(caretAt(editing.layout, p.x, p.y), text.length);
      const stop = (c: string) => (clicks >= 3 ? c === "\n" : !/[\p{L}\p{N}_'’-]/u.test(c));
      let from = i, to = i;
      while (from > 0 && !stop(text[from - 1])) from--;
      while (to < text.length && !stop(text[to])) to++;
      editing.anchor = from;
      editing.caret = to;
    });
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
    if (e.shiftKey && mode === "move") {
      // Shift+click toggles the element in the multi-selection.
      if (selected === null) selected = index;
      else if (selected === index) { selected = also.shift() ?? null; }
      else if (also.includes(index)) also = also.filter((i) => i !== index);
      else also = [...also, index];
      e.stopPropagation();
      return;
    }
    // Dragging an element of the multi-selection moves them all; any other element becomes the selection.
    const moving = mode === "move" && selection.includes(index) ? selection : [index];
    if (moving.length === 1) { also = []; selected = index; }
    drag = { index, startX: e.clientX, startY: e.clientY, rects: new Map(moving.map((i) => [i, { ...view!.elements[i].rect }])), mode };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.stopPropagation();
  }

  /**
   * The offset that brings one of `edges` onto the nearest of `targets`
   * when within `reach`, with the target it lands on.
   */
  function snap(edges: number[], targets: number[], reach: number): { by: number; at: number | null } {
    let best = { by: 0, at: null as number | null }, bestD = reach;
    for (const e of edges) for (const t of targets) {
      const d = Math.abs(t - e);
      if (d < bestD) { bestD = d; best = { by: t - e, at: t }; }
    }
    return best;
  }

  function moveDrag(e: PointerEvent) {
    if (!drag || !view) return;
    let dx = (e.clientX - drag.startX) / scale();
    let dy = (e.clientY - drag.startY) / scale();
    const moving = drag.rects;
    // What a drag snaps to: the slide's edges and center, and the edges
    // and centers of the elements that stay put. Alt turns snapping off.
    const xs = [0, view.width / 2, view.width], ys = [0, view.height / 2, view.height];
    for (const el of view.elements) {
      if (moving.has(el.index)) continue;
      xs.push(el.rect.x, el.rect.x + el.rect.w / 2, el.rect.x + el.rect.w);
      ys.push(el.rect.y, el.rect.y + el.rect.h / 2, el.rect.y + el.rect.h);
    }
    const reach = e.altKey ? 0 : 6 / scale();
    const all = [...moving.values()];
    const x0 = Math.min(...all.map((r) => r.x)), x1 = Math.max(...all.map((r) => r.x + r.w));
    const y0 = Math.min(...all.map((r) => r.y)), y1 = Math.max(...all.map((r) => r.y + r.h));
    if (drag.mode === "move") {
      const sx = snap([x0 + dx, (x0 + x1) / 2 + dx, x1 + dx], xs, reach);
      const sy = snap([y0 + dy, (y0 + y1) / 2 + dy, y1 + dy], ys, reach);
      dx += sx.by; dy += sy.by;
      guides = { v: sx.at === null ? [] : [sx.at], h: sy.at === null ? [] : [sy.at] };
      for (const [i, r] of moving) view.elements[i].rect = { ...r, x: r.x + dx, y: r.y + dy };
    } else {
      const r = moving.get(drag.index)!;
      const sx = snap([r.x + r.w + dx], xs, reach);
      const sy = snap([r.y + r.h + dy], ys, reach);
      dx += sx.by; dy += sy.by;
      guides = { v: sx.at === null ? [] : [sx.at], h: sy.at === null ? [] : [sy.at] };
      // Lines may be flat: a rectangle keeps a minimum size.
      const flat = view.elements[drag.index].shape === "line" || view.elements[drag.index].shape === "arrow";
      const min = flat ? 0 : 10;
      view.elements[drag.index].rect = { ...r, w: Math.max(min, r.w + dx), h: Math.max(min, r.h + dy) };
    }
  }

  async function endDrag() {
    if (!drag || !view) return;
    const indices = [...drag.rects.keys()];
    const moved = indices.some((i) => {
      const a = drag!.rects.get(i)!, b = view!.elements[i].rect;
      return a.x !== b.x || a.y !== b.y || a.w !== b.w || a.h !== b.h;
    });
    drag = null;
    guides = { v: [], h: [] };
    if (!moved) return;
    if (indices.length === 1) await run("Moved", () => api.setRect(indices[0], view!.elements[indices[0]].rect));
    else await run(`Moved ${indices.length} elements`, () => api.setRects(indices.map((i) => [i, view!.elements[i].rect] as [number, Rect])));
  }

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (e.key === "F1") { help = !help; e.preventDefault(); return; }
    if (tag === "TEXTAREA" || tag === "INPUT" || tag === "SELECT") return;
    if (e.key === "?" ) { help = !help; return; }
    if (e.key === "Escape" && help) { help = false; return; }
    if ((e.key === "g" || e.key === "G") && (e.ctrlKey || e.metaKey)) {
      if (e.shiftKey) ungroupSelection(); else groupSelection();
      e.preventDefault();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && (e.key === "z" || e.key === "Z" || e.key === "y" || e.key === "Y") && view?.slide_count) {
      const redo = e.key === "y" || e.key === "Y" || (e.shiftKey && (e.key === "z" || e.key === "Z"));
      undoRedo(redo);
      e.preventDefault();
      return;
    }
    if ((e.key === "v" || e.key === "V") && (e.ctrlKey || e.metaKey) && view?.slide_count) {
      pasteClipboard();
      e.preventDefault();
      return;
    }
    if ((e.key === "c" || e.key === "x" || e.key === "C" || e.key === "X") && (e.ctrlKey || e.metaKey) && !editing && selected !== null) {
      copySelection(e.key.toLowerCase() === "x");
      e.preventDefault();
      return;
    }
    if ((e.key === "b" || e.key === "i") && (e.ctrlKey || e.metaKey) && selected !== null && view) {
      const el = view.elements[selected];
      if (el.kind === "text" || el.kind === "shape") {
        const r = editing ? { from: Math.min(editing.caret, editing.anchor), to: Math.max(editing.caret, editing.anchor) } : null;
        const change = e.key === "b" ? { toggle_bold: true } : { toggle_italic: true };
        enqueue(() => api.styleText(el.index, r?.from ?? null, r?.to ?? null, change).then(() => refreshLayout()));
        e.preventDefault();
        return;
      }
    }
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
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && e.ctrlKey && view) {
      const to = view.slide + (e.key === "ArrowDown" ? 1 : -1);
      if (to >= 0 && to < view.slide_count) run("Moved slide", () => api.moveSlide(view!.slide, to));
      e.preventDefault();
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
    <button onclick={openFile} title="A PowerPoint file, a single-file deck (.deckz) or a deck.json">Open file…</button>
    <button onclick={openFolder}>Open .deck folder…</button>
    <button onclick={save} disabled={!view?.slide_count}>Save</button>
    <button onclick={saveAs} disabled={!view?.slide_count}>Save as…</button>
    <button onclick={exportPptx} disabled={!view?.slide_count}>Export PPTX…</button>
    <button onclick={() => (pdfDialog = true)} disabled={!view?.slide_count}>Export PDF…</button>
    <span class="spacer"></span>
    <button title="Undo (Ctrl+Z)" disabled={!view?.can_undo} onclick={() => undoRedo(false)}>↶</button>
    <button title="Redo (Ctrl+Y)" disabled={!view?.can_redo} onclick={() => undoRedo(true)}>↷</button>
    <button title="Commands and keys (F1)" onclick={() => (help = !help)}>?</button>
    {#if view?.editing_layout !== null && view?.editing_layout !== undefined}
      <span class="banner">Layout
        <input class="layout-name" value={view.editing_layout} onchange={(e) => run("Layout renamed", () => api.renameLayout(e.currentTarget.value))} />
      </span>
      <button onclick={() => run("Added text", api.addText)}>+ Text</button>
      <select class="add-shape" title="Add a shape" value="" onchange={(e) => { const k = e.currentTarget.value as "rectangle" | "ellipse" | "line" | "arrow"; e.currentTarget.value = ""; if (k) enqueue(() => run("Added shape", () => api.addShape(k)).then(selectAdded)); }}>
        <option value="">+ Shape</option>
        <option value="rectangle">▭ Rectangle</option>
        <option value="ellipse">◯ Ellipse</option>
        <option value="line">╱ Line</option>
        <option value="arrow">→ Arrow</option>
      </select>
      <button onclick={() => run("Added placeholder", api.addPlaceholder)}>+ Placeholder</button>
      <button onclick={saveLayoutAs} disabled={!view.dir}>Save layout…</button>
      <button onclick={() => run("Layout deleted", api.deleteLayout)}>Delete layout</button>
      <button class="primary" onclick={() => run("Back to slides", () => api.editLayout(null))}>Done</button>
    {:else if view?.slide_count}
      <button onclick={() => run("Added text", api.addText)}>+ Text</button>
      <select class="add-shape" title="Add a shape" value="" onchange={(e) => { const k = e.currentTarget.value as "rectangle" | "ellipse" | "line" | "arrow"; e.currentTarget.value = ""; if (k) enqueue(() => run("Added shape", () => api.addShape(k)).then(selectAdded)); }}>
        <option value="">+ Shape</option>
        <option value="rectangle">▭ Rectangle</option>
        <option value="ellipse">◯ Ellipse</option>
        <option value="line">╱ Line</option>
        <option value="arrow">→ Arrow</option>
      </select>
      <button onclick={addMedia} title="A video or an animated GIF; images and clips can also be pasted or dropped">+ Video…</button>
      <button onclick={() => run("Added figure", api.addFigure)} title="A Typst figure: CeTZ drawings and plots, edited as source in the panel">+ Figure</button>
      <button onclick={() => run("Added outline", api.addOutline)} title="The sections, the current one highlighted: a plan slide">+ Plan</button>
      {#if selection.length >= 2}<button onclick={groupSelection} title="Group the selected elements (Ctrl+G)">Group</button>{/if}
      {#if sel?.kind === "group"}<button onclick={ungroupSelection} title="Ungroup (Ctrl+Shift+G)">Ungroup</button>{/if}
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
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="stack-item" class:drop-before={dropAt === i} class:drop-after={dropAt === i + 1 && i === thumbnails.length - 1} class:dragging={dragging === i}
             draggable="true" ondragstart={(e) => dragStart(e, i)} ondragover={(e) => dragOver(e, i)} ondragend={dragEnd} ondrop={(e) => { e.preventDefault(); dragEnd(); }}>
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
      {#if sel && sel.kind !== "image" && sel.kind !== "placeholder"}
        <StyleBar element={sel} theme={view.theme}
                  range={editing && editing.index === sel.index ? { from: Math.min(editing.caret, editing.anchor), to: Math.max(editing.caret, editing.anchor) } : null}
                  onrun={(label, f) => enqueue(() => run(label, f).then(() => refreshLayout()))} />
      {/if}
      {#if selection.length && !editing}
        <ArrangeBar elements={selection.map((i) => view!.elements[i]).filter((e) => e && e.kind !== "placeholder")}
                    onrun={(label, f) => enqueue(async () => { await run(label, f); })}
                    onmoved={(ix) => { if (ix.length) { selected = ix[0]; also = ix.slice(1); } }} />
      {/if}
      <div class="canvas-wrap">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="canvas" class:drop-hover={dropHover} bind:this={canvas} style="aspect-ratio: {view.width} / {view.height}"
             onpointermove={moveDrag} onpointerup={endDrag}
             onclick={(e) => { if ((e.target as Element).closest(".hit, .handle, .text-editing")) return; selected = null; also = []; editing = null; }}>
          <SlideView svg={view.svg} width={view.width} height={view.height} dir={view.dir} />
          <svg class="overlay" viewBox="0 0 {view.width} {view.height}" preserveAspectRatio="none">
            {#each view.elements as el (el.index)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <rect class="hit" class:selected={selection.includes(el.index)}
                    x={el.rect.x} y={el.rect.y} width={el.rect.w} height={el.rect.h}
                    onpointerdown={(e) => startDrag(e, el.index, "move")}
                    ondblclick={(e) => { e.stopPropagation(); startEdit(el.index, slidePoint(e)); }} />
              {#if el.index === selected}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <rect class="handle" x={el.rect.x + el.rect.w - 6} y={el.rect.y + el.rect.h - 6} width="12" height="12"
                      onpointerdown={(e) => startDrag(e, el.index, "resize")} />
              {/if}
            {/each}
            {#each guides.v as x}<line class="guide" x1={x} y1="0" x2={x} y2={view.height} />{/each}
            {#each guides.h as y}<line class="guide" x1="0" y1={y} x2={view.width} y2={y} />{/each}
          </svg>
          {#if editing && view.elements[editing.index]}
            {@const r = view.elements[editing.index].rect}
            {@const c = editing.layout.chars[editing.caret]}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <svg class="overlay text-editing" viewBox="0 0 {view.width} {view.height}" preserveAspectRatio="none"
                 onpointerdown={editorPointerDown} onpointermove={editorPointerMove} onpointerup={editorPointerUp} onpointercancel={editorPointerUp}
                 onclick={editorMultiClick}>
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
        {#if view.last_step > 0 || view.step > 0}
          · click {view.step} / {view.last_step}
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
        {#if sel.kind === "media" && sel.media}
          <h3>Playback</h3>
          <label><input type="radio" name="play" checked={sel.media.play === "on_appear"} onchange={() => run("Play on appearance", () => api.styleElement(sel!.index, { play: "appear" }))} /> play when it appears</label>
          <label><input type="radio" name="play" checked={sel.media.play === "button"} onchange={() => run("Play on click", () => api.styleElement(sel!.index, { play: "button" }))} /> play on its button</label>
          <label><input type="checkbox" checked={sel.media.looping} onchange={(e) => run("Loop", () => api.styleElement(sel!.index, { looping: e.currentTarget.checked }))} /> loop (else play once and stop)</label>
          {#if !sel.text.toLowerCase().endsWith(".gif")}
            <label><input type="checkbox" checked={sel.media.sound} onchange={(e) => run("Sound", () => api.styleElement(sel!.index, { sound: e.currentTarget.checked }))} /> sound</label>
          {/if}
          <p class="muted">{sel.text}{sel.media.duration ? ` · ${sel.media.duration.toFixed(1)} s` : ""}. Plays in the presentation windows; the editor shows the first frame. Thumbnails, PDF and PowerPoint exports show the poster.</p>
        {/if}
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
        {:else if sel.kind === "group"}
          <p class="muted">A group of {sel.text}: moves, resizes and animates as one. Ungroup (Ctrl+Shift+G) to edit its parts.</p>
        {:else if sel.kind === "outline"}
          <p class="muted">The plan: every section, the one this slide belongs to in bold, with its subsections. Duplicate this slide and drag a copy to the start of each section; each copy highlights its own.</p>
        {:else if sel.kind === "placeholder"}
          <label>placeholder name
            <input type="text" value={sel.text} onchange={(e) => run("Placeholder renamed", () => api.setText(sel!.index, e.currentTarget.value))} />
          </label>
          <p class="muted">Slides fill this slot by name; "title" and "subtitle" map to PowerPoint's own placeholders.</p>
          {#if sel.placeholder}
            {@const ph = sel.placeholder}
            {@const idx = sel.index}
            <h3>Default text style</h3>
            <div class="theme">
              <label>size <input type="number" min="0" max="200" value={ph.size ?? ""} placeholder={String(view.theme.effective.size ?? 18)}
                     onchange={(e) => run("Placeholder size", () => api.stylePlaceholder(idx, { size: +e.currentTarget.value || 0 }))} /></label>
              <label><input type="checkbox" checked={ph.bold ?? false} onchange={(e) => run("Placeholder bold", () => api.stylePlaceholder(idx, { bold: e.currentTarget.checked }))} /> bold</label>
              <label>color <input type="color" value={ph.color ?? view.theme.effective.color ?? "#000000"} onchange={(e) => run("Placeholder color", () => api.stylePlaceholder(idx, { color: e.currentTarget.value }))} />
                <button title="Theme color" onclick={() => run("Placeholder color reset", () => api.stylePlaceholder(idx, { color: "" }))}>⌀</button></label>
              <label>font
                <select value={ph.font ?? ""} onchange={(e) => run("Placeholder font", () => api.stylePlaceholder(idx, { font: e.currentTarget.value }))}>
                  <option value="">theme font</option>
                  {#each view.theme.fonts as f}<option value={f}>{f}</option>{/each}
                </select>
              </label>
              <label>align
                <select value={ph.align ?? "left"} onchange={(e) => run("Placeholder alignment", () => api.stylePlaceholder(idx, { align: e.currentTarget.value as "left" | "center" | "right" | "justify" }))}>
                  <option value="left">left</option><option value="center">center</option><option value="right">right</option><option value="justify">justify</option>
                </select>
              </label>
              <label><input type="checkbox" checked={ph.bullets ?? false} onchange={(e) => run("Placeholder bullets", () => api.stylePlaceholder(idx, { bullets: e.currentTarget.checked }))} /> bullets</label>
            </div>
            <p class="muted">New slides made from this layout start with this style; existing slides keep theirs.</p>
          {/if}
        {:else if sel.kind !== "image" && sel.kind !== "media"}
          <label>{sel.kind === "math" ? "formula" : sel.kind === "figure" ? "Typst source" : "text"}
            <textarea rows={sel.kind === "figure" ? 12 : 6} class:code={sel.kind === "figure" || sel.kind === "math"} value={sel.text} onchange={(e) => api.setText(sel!.index, e.currentTarget.value)}></textarea>
          </label>
          {#if sel.kind === "figure"}
            <p class="muted">Any Typst markup, with <code>@preview/cetz:0.5.2</code> and <code>@preview/cetz-plot:0.1.4</code> bundled: <code>plot.add(points)</code>, <code>plot.add-bar</code>, <code>draw.line</code>… The page is as wide as the box and is scaled to fit it. A Python macro can generate it from data.</p>
          {/if}
        {/if}
        {#if sel.kind !== "placeholder" && view.editing_layout === null}
          <h3>Build steps</h3>
          <Steps element={sel} current={view.step} last={view.last_step}
                 onrun={(label, f) => run(label, f)} ongoto={(step) => api.goto(view!.slide, step)} />
        {/if}
      {:else}
        <p class="muted">Click an element to select it, Shift+click to select several (then Group), double-click its text to edit it in place (arrows, Shift for a selection, Enter for a paragraph, Escape to finish). Drag to move, corner handle to resize, Delete to remove.</p>
      {/if}
      {#if view.editing_layout === null}
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
      {/if}
      <h3>Layout</h3>
      {#if view.editing_layout !== null}
        <p class="muted">Static elements of the layout are drawn under every slide that uses it; placeholders (dashed) are the slots new slides start from. In texts, these variables are filled per slide: <code>{"{{section}}"}</code>, <code>{"{{subsection}}"}</code>, <code>{"{{slide}}"}</code>, <code>{"{{slides}}"}</code>, <code>{"{{title}}"}</code>, <code>{"{{deck}}"}</code>, <code>{"{{date}}"}</code>.</p>
      {:else}
        {#if view.layouts.length}
          <select value={view.layout ?? ""} onchange={(e) => run("Layout set", () => api.setLayout(e.currentTarget.value || null))}>
            <option value="">none</option>
            {#each view.layouts as name}<option value={name}>{name}</option>{/each}
          </select>
          <p class="muted">"+ Slide" starts from this layout's placeholders.</p>
        {:else}
          <p class="muted">No layouts in this deck. Importing a PPTX brings its masters along.</p>
        {/if}
        <div class="row">
          {#if view.layout}<button onclick={() => run("Editing layout", () => api.editLayout(view!.layout))}>Edit layout</button>{/if}
          <button onclick={() => run("New layout", api.newLayout)}>+ New layout</button>
          <button onclick={loadLayoutFrom} disabled={!view.dir}>Load layout…</button>
        </div>
      {/if}
      <h3>Theme{view.editing_layout !== null ? " of this layout" : " of the deck"}</h3>
      <p class="muted">
        {#if view.editing_layout !== null}
          Default font, size and colors of the slides using this layout; blank fields take the deck's.
        {:else}
          Default font, size and colors of text that sets none; a layout's theme overrides them.
          Fonts in the deck's <code>fonts/</code> folder are listed here.
        {/if}
      </p>
      <div class="theme">
        <label>font
          <select value={view.theme.own.font ?? ""} onchange={(e) => run("Theme font", () => api.setTheme({ font: e.currentTarget.value }))}>
            <option value="">{view.editing_layout !== null && view.theme.effective.font ? `deck (${view.theme.effective.font})` : "DejaVu Sans"}</option>
            {#each view.theme.fonts as f}<option value={f}>{f}</option>{/each}
          </select>
        </label>
        <label>size
          <input type="number" min="0" max="200" step="1" value={view.theme.own.size ?? ""} placeholder={String(view.theme.effective.size ?? 18)}
                 onchange={(e) => run("Theme size", () => api.setTheme({ size: +e.currentTarget.value || 0 }))} />
        </label>
        <label>text
          <input type="color" value={view.theme.effective.color ?? "#000000"} onchange={(e) => run("Theme text color", () => api.setTheme({ color: e.currentTarget.value }))} />
          <button title="Inherit" onclick={() => run("Theme text color reset", () => api.setTheme({ color: "" }))}>⌀</button>
        </label>
        <label>accent
          <input type="color" value={view.theme.effective.accent ?? "#000000"} onchange={(e) => run("Theme accent", () => api.setTheme({ accent: e.currentTarget.value }))} />
          <button title="Inherit" onclick={() => run("Theme accent reset", () => api.setTheme({ accent: "" }))}>⌀</button>
        </label>
        <div class="palette">palette
          {#each view.theme.own.palette ?? [] as c, i}
            <span class="swatch-edit">
              <input type="color" value={c} title="Swatch {i + 1}; click to change"
                     onchange={(e) => run("Palette", () => api.setTheme({ palette: (view!.theme.own.palette ?? []).map((x, j) => (j === i ? e.currentTarget.value : x)) }))} />
              <button title="Remove swatch" onclick={() => run("Palette", () => api.setTheme({ palette: (view!.theme.own.palette ?? []).filter((_, j) => j !== i) }))}>×</button>
            </span>
          {/each}
          <button title="Add a swatch" onclick={() => run("Palette", () => api.setTheme({ palette: [...(view!.theme.own.palette ?? []), view!.theme.effective.accent ?? "#4472C4"] }))}>+</button>
          {#if view.editing_layout !== null && !(view.theme.own.palette ?? []).length && (view.theme.effective.palette ?? []).length}
            <span class="muted">(deck's)</span>
          {/if}
        </div>
      </div>
      <h3>Macros</h3>
      {#if scripts?.python}
        {#if scripts.scripts.length}
          <div class="macros">
            {#each scripts.scripts as name}
              <button onclick={() => runMacro(name)} title="scripts/{name} of this deck">▶ {name.replace(/\.py$/, "")}</button>
            {/each}
          </div>
        {:else}
          <p class="muted">Python scripts in the deck's <code>scripts/</code> folder are listed here.</p>
        {/if}
        <div class="row"><button onclick={runScriptFile}>Run script…</button></div>
        {#if scriptOutput}<pre class="output">{scriptOutput}</pre>{/if}
      {:else}
        <p class="muted">No Python found on this machine. Install one (python.org, conda) or point <code>DECKER_PYTHON</code> at it to run macros.</p>
      {/if}
      {#if view.editing_layout === null}
      <h3>Notes</h3>
      <textarea rows="8" value={view.notes} onchange={(e) => api.setNotes(e.currentTarget.value)}></textarea>
      {/if}
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
  {#if help}<Help onclose={() => (help = false)} />{/if}

  {#if pdfDialog}
    <div class="modal">
      <div class="dialog small">
        <h3>Export PDF</h3>
        <label><input type="radio" name="pdfmode" checked={!pdfSteps} onchange={() => (pdfSteps = false)} /> One page per slide, as it looks at the end (for reading and printing)</label>
        <label><input type="radio" name="pdfmode" checked={pdfSteps} onchange={() => (pdfSteps = true)} /> One page per build step (for presenting with decker-viewer or any PDF reader)</label>
        <p class="muted">Sections and slide titles become bookmarks. Speaker notes are written next to the PDF as <code>name.notes.md</code>, which decker-viewer shows in its presenter view.</p>
        <div class="row">
          <button class="primary" onclick={exportPdf}>Choose file…</button>
          <button onclick={() => (pdfDialog = false)}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}
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
  header { grid-area: head; display: flex; flex-wrap: wrap; gap: 6px; padding: 8px; border-bottom: 1px solid var(--border); align-items: center; }
  header button { white-space: nowrap; }
  .spacer { flex: 1; }
  .banner { color: var(--muted); display: flex; align-items: center; gap: 6px; }
  .layout-name { width: 160px; padding: 3px 6px; }
  .row { display: flex; gap: 6px; margin-top: 6px; }
  .macros { display: flex; flex-wrap: wrap; gap: 6px; }
  .dialog.small { width: min(520px, 90vw); }
  .guide { stroke: #ff3b8d; stroke-width: 1; vector-effect: non-scaling-stroke; stroke-dasharray: 4 3; pointer-events: none; }
  .add-shape { width: auto; flex: none; padding: 5px 6px; background: var(--panel); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; }
  .text-editing { user-select: none; }
  textarea.code { font-family: ui-monospace, "DejaVu Sans Mono", monospace; font-size: 11px; white-space: pre; }
  .output { font-size: 11px; max-height: 160px; overflow: auto; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 6px; white-space: pre-wrap; margin: 6px 0 0; }
  .theme { display: flex; flex-wrap: wrap; gap: 6px 12px; align-items: center; font-size: 12px; color: var(--muted); }
  .theme label { display: flex; align-items: center; gap: 4px; }
  .theme input[type="number"] { width: 58px; padding: 2px 4px; }
  .theme input[type="color"] { width: 28px; height: 22px; padding: 0; border: 1px solid var(--border); background: none; }
  .theme button { padding: 1px 6px; min-width: 22px; }
  .palette { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; width: 100%; }
  .swatch-edit { display: inline-flex; align-items: center; }
  code { background: var(--bg); padding: 0 3px; border-radius: 3px; font-size: 11px; }
  .slides { grid-area: slides; overflow-y: auto; padding: 8px; border-right: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  .stack-item { position: relative; border-top: 2px solid transparent; border-bottom: 2px solid transparent; cursor: grab; }
  .stack-item.dragging { opacity: 0.4; }
  .stack-item.drop-before { border-top-color: var(--accent); }
  .stack-item.drop-after { border-bottom-color: var(--accent); }
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
  .canvas.drop-hover { outline: 3px dashed var(--accent); outline-offset: 4px; }
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
