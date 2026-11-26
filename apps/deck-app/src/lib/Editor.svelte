<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import lockup from "../assets/decker_lockup.svg";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, watchView, watchThumbnails, presentationKeys, type View, type Rect, type Crop, type Mask, type TextLayout } from "./api";
  import { SlideView, captureFrame } from "$shared";
  import StyleBar from "./StyleBar.svelte";
  import PathTool from "./PathTool.svelte";
  import Timeline from "./Timeline.svelte";
  import MenuButton from "./MenuButton.svelte";
  import Variables from "./Variables.svelte";
  import type { PathNode } from "./paths";
  import ArrangeBar from "./ArrangeBar.svelte";
  import PlaceholderFields from "./PlaceholderFields.svelte";
  import TablePanel from "./TablePanel.svelte";
  import Help from "./Help.svelte";
  import Steps from "./Steps.svelte";

  let help = $state(false);

  let view = $state<View | null>(null);
  /** A layout or a theme is under edit, not a slide. */
  const editingTemplate = $derived(view !== null && (view.editing_layout !== null || view.editing_theme !== null));
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
      // Slides added or removed: the picked indices no longer mean the same slides.
      if (view && v.slide_count !== view.slide_count) picked = [];
      view = v;
      if (selected !== null && selected >= v.elements.length) selected = null;
      also = also.filter((i) => i < v.elements.length);
      if (editing && (editing.index >= v.elements.length || v.slide !== editingSlide || v.editing_layout !== editingLayout || v.editing_theme !== editingTheme)) editing = null;
      // Another canvas (a layout, a theme, back to the slides): the selection meant other elements.
      if (v.editing_layout !== editingLayout || v.editing_theme !== editingTheme) { selected = null; also = []; }
      editingSlide = v.slide;
      editingLayout = v.editing_layout;
      editingTheme = v.editing_theme;
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

  /** The poster of the selected video: the frame at `posterAt` seconds, captured from the canvas. */
  let posterAt = $state(0);
  function capturePoster(index: number, src: string) {
    const video = Array.from(canvas?.querySelectorAll<HTMLVideoElement>("video[data-media]") ?? []).find((v) => v.dataset.media === src);
    if (!video) { status = "The video is not on the canvas"; return; }
    enqueue(async () => {
      try {
        const png = await captureFrame(video, posterAt);
        await run("Poster set", () => api.setPoster(index, png));
      } catch (e) {
        status = `Could not capture the frame: ${e instanceof Error ? e.message : e}`;
      }
    });
  }

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
  // ----- the canvas menu (right-click an element) -----
  let elementMenu = $state<{ x: number; y: number } | null>(null);
  function openElementMenu(e: MouseEvent, index: number) {
    e.preventDefault();
    e.stopPropagation();
    if (!selection.includes(index)) { selected = index; also = []; }
    elementMenu = { x: e.clientX, y: Math.min(e.clientY, window.innerHeight - 420) };
  }
  function onElement(f: () => unknown) {
    elementMenu = null;
    f();
  }
  /** Ctrl+D: a copy of the selection, through the clipboard. */
  function duplicateSelection() {
    if (!selection.length) return;
    const indices = [...selection];
    enqueue(async () => { await api.copyElements(indices); });
    pasteClipboard();
  }
  function deleteSelection() {
    const indices = [...selection].sort((a, b) => b - a);
    selected = null;
    also = [];
    enqueue(async () => { await run(indices.length > 1 ? `Deleted ${indices.length} elements` : "Deleted element", async () => { for (const i of indices) await api.deleteElement(i); }); });
  }
  function orderSelection(op: "front" | "forward" | "backward" | "back") {
    const indices = [...selection];
    enqueue(async () => {
      const moved = await run("Order", () => api.arrange(indices, op));
      if (moved && moved.length) { selected = moved[0]; also = moved.slice(1); }
    });
  }
  /** Appears (or disappears) at a new click at the end of the slide's timeline. */
  function animateSelection(appear: boolean) {
    if (!view || selected === null) return;
    const el = view.elements[selected];
    const at = view.last_step + 1;
    let steps = el.steps.map((s) => ({ ...s, set: { ...s.set } }));
    if (appear) {
      steps = steps.filter((s) => !(s.at === 0 && s.set.opacity === 0));
      steps.unshift({ at: 0, set: { opacity: 0 } });
    }
    steps.push({ at, set: { opacity: appear ? 1 : 0 }, duration: 0.3 });
    const i = el.index;
    run(appear ? `Appears at click ${at}` : `Disappears at click ${at}`, () => api.setSteps(i, steps));
  }
  function renameSelection() {
    panelOpen = true;
    panelTab = "element";
    queueMicrotask(() => { const input = document.querySelector<HTMLInputElement>(".props label.name input"); input?.focus(); input?.select(); });
  }
  function present(fromStart: boolean) {
    enqueue(async () => {
      if (fromStart && view) await api.goto(0, 0);
      await run("Presenting", api.startPresentation);
    });
  }

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

  // ----- the design view: themes and layouts, like PowerPoint's master view -----
  type DesignItem = { kind: "theme" | "layout"; name: string; theme: string | null; svg: string; used: number };
  let tree = $state<DesignItem[]>([]);
  let treeMenu = $state<{ item: DesignItem | null; x: number; y: number } | null>(null);
  $effect(() => {
    if (view && editingTemplate) {
      void view.svg; // refreshed with every change
      api.designTree().then((t) => (tree = t)).catch(() => {});
    }
  });
  /** Opens the design view on the current slide's layout (else its theme). */
  function enterDesign() {
    if (!view) return;
    const layout = view.layout;
    const theme = view.theme.of_theme ?? view.layout_groups.find((g) => g.theme)?.theme ?? null;
    enqueue(async () => {
      if (layout) await run("Design view", () => api.editLayout(layout));
      else if (theme) await run("Design view", () => api.editTheme(theme));
      else await run("Design view", api.newTheme);
    });
  }
  function exitDesign() {
    enqueue(async () => {
      await api.editLayout(null);
      await run("Back to the slides", () => api.editTheme(null));
    });
  }
  function openDesignItem(item: DesignItem) {
    run(item.kind === "theme" ? "Editing theme" : "Editing layout", () => (item.kind === "theme" ? api.editTheme(item.name) : api.editLayout(item.name)));
  }
  function openTreeMenu(e: MouseEvent, item: DesignItem | null) {
    e.preventDefault();
    e.stopPropagation();
    treeMenu = { item, x: e.clientX, y: Math.min(e.clientY, window.innerHeight - 320) };
  }
  const PRESETS: [string, string][] = [
    ["title", "Title slide"], ["content", "Title and content"], ["section", "Section header"], ["two", "Two contents"],
    ["compare", "Comparison"], ["title-only", "Title only"], ["blank", "Blank"],
  ];
  /** A placeholder being renamed on the canvas. */
  let phRename = $state<{ index: number; value: string } | null>(null);
  function commitPhRename() {
    const r = phRename;
    phRename = null;
    if (!r || !view || !r.value.trim() || r.value.trim() === view.elements[r.index]?.text) return;
    run("Placeholder renamed", () => api.setText(r.index, r.value.trim()));
  }
  /** Inserts a {{ }} variable where text is being typed: the last focused
   *  prefix field, or the text box being edited. False when neither. */
  let varTarget: HTMLInputElement | null = null;
  function insertVariable(text: string): boolean {
    if (editing) {
      const lo = Math.min(editing.caret, editing.anchor), hi = Math.max(editing.caret, editing.anchor);
      enqueue(() => applyEdit(lo, hi, text));
      return true;
    }
    const t = varTarget;
    if (t && document.body.contains(t)) {
      const at = t.selectionStart ?? t.value.length, end = t.selectionEnd ?? at;
      t.value = t.value.slice(0, at) + text + t.value.slice(end);
      t.setSelectionRange(at + text.length, at + text.length);
      t.dispatchEvent(new Event("change", { bubbles: true }));
      t.focus();
      return true;
    }
    return false;
  }
  function trackVarTarget(e: FocusEvent) {
    const t = e.target as HTMLElement;
    if (t instanceof HTMLInputElement && t.classList.contains("vars-target")) varTarget = t;
  }

  /** A theme or layout being renamed from the tree's menu. */
  let renamingItem = $state<{ kind: "theme" | "layout"; name: string; value: string } | null>(null);
  function renameItem() {
    const r = renamingItem;
    renamingItem = null;
    if (!r || !r.value.trim() || r.value.trim() === r.name) return;
    const value = r.value.trim();
    enqueue(async () => {
      if (r.kind === "theme") {
        await api.editTheme(r.name);
        await run("Theme renamed", () => api.renameTheme(value));
      } else {
        await api.editLayout(r.name);
        await run("Layout renamed", () => api.renameLayout(value));
      }
    });
  }
  function treeAction(f: () => unknown) {
    treeMenu = null;
    f();
  }

  /** Whether step `st` of element `index` is the first effect of its click. */
  function firstOfClick(index: number, st: { at: number; seq?: number }): boolean {
    if (!view) return true;
    const k = view.elements[index]?.steps.indexOf(st as never) ?? 0;
    const key = (e: number, i: number, q: number) => [q, e, i];
    const mine = key(index, k, st.seq ?? 0);
    return !view.elements.some((el) => el.steps.some((o, i) => {
      if (o.at !== st.at || (el.index === index && i === k)) return false;
      const other = key(el.index, i, o.seq ?? 0);
      return other[0] < mine[0] || (other[0] === mine[0] && (other[1] < mine[1] || (other[1] === mine[1] && other[2] < mine[2])));
    }));
  }

  /** The arrange tools, unfolded from their button. */
  let arrangeOpen = $state(false);
  // ----- unsaved changes -----
  /** What waits for an answer about the unsaved changes. */
  let pending = $state<{ what: string; action: () => unknown } | null>(null);
  /** Runs `action`, asking first when the deck has unsaved changes. */
  function guarded(action: () => unknown, what = "Continue") {
    if (view?.dirty) pending = { what, action };
    else action();
  }
  async function answer(choice: "save" | "discard" | "cancel") {
    const p = pending;
    pending = null;
    if (!p || choice === "cancel") return;
    if (choice === "save") {
      await save();
      if (view?.dirty) return; // the save was cancelled or failed
    }
    p.action();
  }
  onMount(() => {
    // Closing the window with unsaved changes: the backend asks us first.
    const un = listen("close-requested", () => { pending = { what: "Close", action: api.quit }; });
    return () => { un.then((f) => f()); };
  });

  /** The File menu, open. */
  let fileMenu = $state(false);
  function fileAction(f: () => unknown) {
    fileMenu = false;
    f();
  }

  async function openFile() {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Presentations", extensions: ["pptx", "pptm", "deckz", "json"] }],
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
  let pdfMode = $state<"slides" | "steps" | "notes">("slides");
  async function exportPdf() {
    pdfDialog = false;
    const picked = await saveDialog({ defaultPath: "talk.pdf", filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!picked) return;
    await run(`Exported ${picked}`, () => api.exportPdf(picked, pdfMode));
  }

  // Reordering slides by dragging thumbnails: the drop target is the gap
  // before or after a thumbnail, from the pointer's vertical position.
  let dragging = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  // Thumbnails are reordered with pointer events: HTML5 drag events never
  // reach the page on Windows while the window handles file drops.
  let pressed: { slide: number; y: number } | null = null;
  /** A drag just ended: the click that follows must not open the slide. */
  let dragged = false;

  function thumbDown(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    pressed = { slide: i, y: e.clientY };
  }
  function thumbMove(e: PointerEvent) {
    if (!pressed) return;
    if (dragging === null) {
      // A few pixels first, so that a click stays a click.
      if (Math.abs(e.clientY - pressed.y) < 6) return;
      dragging = pressed.slide;
    }
    const item = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>(".stack-item[data-index]");
    if (!item) return;
    const i = Number(item.dataset.index);
    const r = item.getBoundingClientRect();
    dropAt = e.clientY < r.top + r.height / 2 ? i : i + 1;
  }
  function thumbUp() {
    if (dragging !== null && dropAt !== null) {
      const from = dragging;
      // Dropping after the dragged slide: the index shifts once it is removed.
      const to = dropAt > from ? dropAt - 1 : dropAt;
      if (to !== from) enqueue(async () => { await run(`Moved slide ${from + 1} to ${to + 1}`, () => api.moveSlide(from, to)); });
      dragged = true;
    }
    pressed = null;
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
  // The name field of a new heading appears once the view has refreshed: focus it then.
  let focusedRename: HTMLInputElement | undefined;
  $effect(() => {
    const input = renameInput;
    if (renaming && input && input !== focusedRename) {
      focusedRename = input;
      input.focus();
      input.select();
    }
    if (!renaming) focusedRename = undefined;
  });

  // ----- The thumbnail pane's right-click menu
  let slideMenu = $state<{ slide: number; x: number; y: number } | null>(null);
  /** Thumbnails picked with Ctrl+click or Shift+click: the slides the menu acts on. */
  let picked = $state<number[]>([]);
  /** The layout list of the slide menu, unfolded. */
  let layoutList = $state(false);
  function clickThumb(e: MouseEvent, i: number) {
    if (dragged) { dragged = false; return; }
    if (!view) return;
    if (e.ctrlKey || e.metaKey) {
      const base = picked.length ? picked : [view.slide];
      picked = base.includes(i) ? base.filter((k) => k !== i) : [...base, i].sort((a, b) => a - b);
      return;
    }
    if (e.shiftKey) {
      const [a, b] = [Math.min(view.slide, i), Math.max(view.slide, i)];
      picked = Array.from({ length: b - a + 1 }, (_, k) => a + k);
      return;
    }
    picked = [];
    api.goto(i);
  }
  /** The slides a menu opened on `slide` acts on: the picked ones when it is among them. */
  const targets = (slide: number) => (picked.includes(slide) ? picked : [slide]);
  function applyLayout(name: string | null) {
    const slides = targets(slideMenu!.slide);
    slideMenu = null;
    layoutList = false;
    enqueue(async () => { await run(slides.length > 1 ? `Layout of ${slides.length} slides` : "Layout", () => api.setLayoutOf(slides, name)); });
  }

  function openSlideMenu(e: MouseEvent, slide: number) {
    e.preventDefault();
    layoutList = false;
    // Kept inside the window: the menu is about 300 px tall.
    slideMenu = { slide, x: e.clientX, y: Math.min(e.clientY, window.innerHeight - 310) };
  }
  /** Runs a slide action on the slide the menu was opened on. */
  function onSlide(label: string, f: () => Promise<unknown>) {
    const slide = slideMenu!.slide;
    slideMenu = null;
    enqueue(() => run(label, async () => { await api.goto(slide); await f(); }));
  }
  /** Right-click on the pane between or below slides: where a new slide would go. */
  let gapMenu = $state<{ at: number; x: number; y: number } | null>(null);
  function openGapMenu(e: MouseEvent) {
    if (e.defaultPrevented || !view) return; // a thumbnail's own menu
    e.preventDefault();
    const items = Array.from((e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(".stack-item[data-index]"));
    const after = items.find((el) => { const r = el.getBoundingClientRect(); return e.clientY < r.top + r.height / 2; });
    gapMenu = { at: after ? Number(after.dataset.index) : view.slide_count, x: e.clientX, y: Math.min(e.clientY, window.innerHeight - 150) };
  }
  /** A new slide at position `at` (from the layout of the slide before it); returns its index. */
  async function slideAt(at: number): Promise<number> {
    await api.goto(Math.max(at - 1, 0));
    await api.addSlide();
    if (at === 0) await api.moveSlide(1, 0);
    return at;
  }
  function onGap(what: "slide" | "plan" | "section" | "subsection") {
    const at = gapMenu!.at;
    gapMenu = null;
    enqueue(async () => {
      await run(what === "slide" ? "Added slide" : what === "plan" ? "Added plan slide" : `Added ${what}`, async () => {
        const k = await slideAt(at);
        if (what === "plan") {
          await api.goto(k);
          await api.addOutline();
        } else if (what !== "slide") {
          await api.setSectionOf(k, what === "section" ? "Section" : null, what === "subsection" ? "Subsection" : null);
          renaming = { slide: k, level: what };
          queueMicrotask(() => { renameInput?.focus(); renameInput?.select(); });
        }
      });
    });
  }

  function removeHeading(slide: number, level: "section" | "subsection") {
    const st = view!.starts[slide];
    slideMenu = null;
    // A section removed takes its subsection start along only when the slide starts no subsection of its own.
    const section = level === "section" ? null : st.section;
    const subsection = level === "subsection" ? null : st.subsection;
    enqueue(async () => { await run(`${level === "section" ? "Section" : "Subsection"} removed`, () => api.setSectionOf(slide, section, subsection)); });
  }

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
  let drag: { index: number; startX: number; startY: number; rects: Map<number, Rect>; mode: "move" | "resize" | "rotate" } | null = null;
  /** The rubber band dragged over the empty canvas, in slide coordinates. */
  let band = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  /** A band was just dragged: the click that ends it must not clear the selection. */
  let banded = false;
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
  let editingTheme: string | null = null;

  /** The table cell being edited in place, when the edited element is a table. */
  let editingCell: { row: number; column: number } | null = null;

  /** Tab in a table: on to the next cell (Shift: the previous one), wrapping over rows. */
  async function nextCell(back: boolean) {
    if (!editing || !editingCell || !view) return;
    const table = view.elements[editing.index]?.table;
    if (!table) return;
    const columns = table.rects[0]?.length ?? 1, count = table.rects.length * columns;
    // Cells covered by a merge are skipped.
    let k = editingCell.row * columns + editingCell.column;
    for (let n = 0; n < count; n++) {
      k = (k + (back ? count - 1 : 1)) % count;
      if (table.spans[Math.floor(k / columns)]?.[k % columns]?.[0] !== 0) break;
    }
    editingCell = { row: Math.floor(k / columns), column: k % columns };
    await api.focusCell(editing.index, editingCell.row, editingCell.column);
    const layout = await api.textLayout(editing.index);
    if (!editing || !layout) { editing = null; return; }
    // The whole cell selected, as a spreadsheet does.
    editing = { ...editing, layout, anchor: 0, caret: layout.chars.length - 1 };
  }

  // ----- panels: room for the slide -----
  /** Per-viewer settings, remembered across sessions when storage works. */
  function remembered<T>(key: string, fallback: T): T {
    try { const v = localStorage.getItem("decker." + key); return v === null ? fallback : (JSON.parse(v) as T); } catch { return fallback; }
  }
  function remember(key: string, value: unknown) {
    try { localStorage.setItem("decker." + key, JSON.stringify(value)); } catch { /* not kept */ }
  }
  /** The right panel: open or collapsed to a strip, and its tab. */
  let panelOpen = $state(remembered("panelOpen", true));
  let panelTab = $state<"element" | "slide" | "anim">(((t) => (t === "props" ? "element" : t))(remembered<string>("panelTab", "slide")) as "element" | "slide" | "anim");
  /** The notes under the slide: unfolded or not. */
  let notesOpen = $state(remembered("notesOpen", true));
  /** Zoom of the canvas: `null` fits the slide in the window, else a factor
      of the slide's size in points (1 = 100%). */
  let zoom = $state<number | null>(null);
  const ZOOMS = [0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4];
  function currentZoom(): number {
    return view && canvas ? canvas.getBoundingClientRect().width / view.width : 1;
  }
  function zoomBy(dir: 1 | -1) {
    const z = currentZoom();
    const next = dir > 0 ? ZOOMS.find((v) => v > z + 0.01) : [...ZOOMS].reverse().find((v) => v < z - 0.01);
    zoom = next ?? (dir > 0 ? ZOOMS[ZOOMS.length - 1] : ZOOMS[0]);
  }
  let zoomShown = $state(100);
  /** The room for the slide, and the width that fits it there. */
  let wrapW = $state(0), wrapH = $state(0);
  const fitWidth = $derived(view ? Math.max(100, Math.min(wrapW - 32, ((wrapH - 32) * view.width) / view.height)) : 100);
  $effect(() => {
    void zoom; void view; void panelOpen; void slidesOpen; void slidesWidth;
    requestAnimationFrame(() => (zoomShown = Math.round(currentZoom() * 100)));
  });

  /** The thumbnail pane: its width (dragging its edge) and whether it is open. */
  let slidesWidth = $state(remembered("slidesWidth", 180));
  let slidesOpen = $state(remembered("slidesOpen", true));
  $effect(() => {
    remember("panelOpen", panelOpen); remember("panelTab", panelTab); remember("notesOpen", notesOpen);
    remember("slidesWidth", slidesWidth); remember("slidesOpen", slidesOpen);
  });
  // A narrow window: the side panel collapses when the window becomes
  // narrow (it can still be opened by hand).
  const NARROW = 1250;
  let wasNarrow = typeof window !== "undefined" && window.innerWidth < NARROW;
  if (wasNarrow) panelOpen = false;
  function onResize() {
    const narrow = window.innerWidth < NARROW;
    if (narrow && !wasNarrow) panelOpen = false;
    wasNarrow = narrow;
  }
  let resizingSlides: { x: number; w: number } | null = null;
  function startResizeSlides(e: PointerEvent) {
    e.preventDefault();
    resizingSlides = { x: e.clientX, w: slidesWidth };
  }
  function resizeSlides(e: PointerEvent) {
    if (resizingSlides) slidesWidth = Math.round(Math.min(360, Math.max(110, resizingSlides.w + e.clientX - resizingSlides.x)));
  }
  // Selecting shows the element's tab, deselecting the slide's (the
  // Animations tab stays where it is).
  let hadSelection = false;
  $effect(() => {
    const has = selected !== null;
    if (has !== hadSelection) {
      if (has && panelTab === "slide") panelTab = "element";
      if (!has && panelTab === "element") panelTab = "slide";
      hadSelection = has;
    }
  });
  // The design view has no Animations tab.
  $effect(() => { if (editingTemplate && panelTab === "anim") panelTab = selected !== null ? "element" : "slide"; });
  function showTab(tab: "element" | "slide" | "anim") {
    if (panelOpen && panelTab === tab) panelOpen = false;
    else { panelTab = tab; panelOpen = true; }
  }

  /** Drawing a path (the Shape menu's freehand and Bézier tools). */
  let pathMode = $state<"free" | "bezier" | null>(null);
  /** The path whose nodes are being edited (double-click on a path). */
  let pathEdit = $state<number | null>(null);
  const SHAPES = [
    { value: "rectangle", label: "▭ Rectangle", key: "R" },
    { value: "ellipse", label: "◯ Ellipse" },
    { value: "line", label: "╱ Line" },
    { value: "arrow", label: "→ Arrow" },
    { value: "free", label: "✎ Freehand", title: "Drag to draw; end near the start to close it" },
    { value: "bezier", label: "✒ Bézier path", title: "Click for corners, click and drag for curves; click the first point to close" },
  ];
  function addShapeOf(k: string) {
    if (k === "free" || k === "bezier") {
      selected = null;
      also = [];
      pathMode = k;
      status = k === "free" ? "Drag to draw; end near the start to close the shape" : "Click for corners, drag for curves; click the first point to close, Enter to end";
    } else if (k) {
      enqueue(() => run("Added shape", () => api.addShape(k as "rectangle" | "ellipse" | "line" | "arrow")).then(selectAdded));
    }
  }
  function pathDrawn(nodes: PathNode[], closed: boolean) {
    enqueue(async () => {
      const index = await run("Added path", () => api.addPath(nodes, closed));
      if (typeof index === "number") selected = index;
    });
  }
  function pathEdited(nodes: PathNode[], closed: boolean) {
    const index = pathEdit;
    if (index === null) return;
    enqueue(async () => { await run("Path edited", () => api.setPath(index, nodes, closed)); });
  }
  /** Slide points per screen pixel on the canvas. */
  const unit = () => (view && canvas ? view.width / Math.max(canvas.getBoundingClientRect().width, 1) : 1);

  async function startEdit(index: number, at?: { x: number; y: number }) {
    if (!view) return;
    const el = view.elements[index];
    if (el.shape === "path" && el.path) {
      selected = index;
      pathEdit = index;
      status = "Drag points and handles; double-click a point for a corner or a curve; Delete removes it; Escape ends";
      return;
    }
    if (el.kind === "placeholder") {
      // A placeholder's name is typed in place, over its box.
      selected = index;
      phRename = { index, value: el.text };
      queueMicrotask(() => { const i = document.querySelector<HTMLInputElement>(".ph-rename"); i?.focus(); i?.select(); });
      return;
    }
    if (el.kind === "image" || el.kind === "math" || el.kind === "outline" || el.kind === "media" || el.kind === "figure") return;
    selected = index;
    if (el.kind === "table" && el.table) {
      // The cell under the click (the first one otherwise) takes the text edits.
      let cell = { row: 0, column: 0 };
      if (at) {
        el.table.rects.forEach((row, i) => row.forEach((r, j) => {
          if (at.x >= r.x && at.x <= r.x + r.w && at.y >= r.y && at.y <= r.y + r.h) cell = { row: i, column: j };
        }));
      }
      await api.focusCell(index, cell.row, cell.column);
      editingCell = cell;
    } else editingCell = null;
    const layout = await api.textLayout(index);
    if (!layout) { status = "Text editing in place needs the Typst feature"; return; }
    const caret = at ? caretAt(layout, at.x, at.y) : layout.chars.length - 1;
    editing = { index, caret, anchor: caret, layout };
  }

  // The caret goes to Rust, which shows the formula around it as source.
  let sentCaret = "";
  $effect(() => {
    const key = editing ? `${editing.index}:${editing.caret}` : "";
    if (key === sentCaret) return;
    sentCaret = key;
    const index = editing?.index ?? null, caret = editing?.caret ?? 0;
    enqueue(async () => { if (await api.setCaret(index, caret)) await refreshLayout(); });
  });

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
    const x = caretGoalX ?? layout.chars[Math.min(caret, layout.chars.length - 1)].x;
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
    // The caret moves once the new layout is in: an index past the old
    // layout's end would leave the caret nowhere to be drawn.
    await refreshLayout();
    if (editing) { editing.caret = editing.anchor = Math.min(caret, editing.layout.chars.length - 1); }
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
      case "Tab": return editingCell ? () => nextCell(shift) : null;
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
    // Alt+= : an inline formula, the caret between its dollars.
    if (key === "=" && alt && !ctrl) return withState(async ({ lo, hi }) => { await applyEdit(lo, hi, "$$"); if (editing) { editing.caret = editing.anchor = lo + 1; } });
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
    for (let i = lo; i < Math.min(hi, ed.layout.chars.length); i++) {
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

  function startDrag(e: PointerEvent, index: number, mode: "move" | "resize" | "rotate") {
    // Only the main button drags: a right-click opens the menu.
    if (!view || e.button !== 0) return;
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

  /** Pressing on the empty canvas starts a rubber band. */
  function startBand(e: PointerEvent) {
    if (!view || editing || e.button !== 0 || (e.target as Element).closest(".hit, .handle, .rotate, .text-editing")) return;
    const p = slidePoint(e);
    band = { x0: p.x, y0: p.y, x1: p.x, y1: p.y };
    // Captured, so that releasing outside the canvas still ends the band.
    canvas?.setPointerCapture(e.pointerId);
  }

  function moveDrag(e: PointerEvent) {
    if (band && view) {
      const p = slidePoint(e);
      band = { ...band, x1: p.x, y1: p.y };
      return;
    }
    if (!drag || !view) return;
    if (drag.mode === "rotate") {
      // The angle from the box's center to the pointer; straight up is 0. Shift snaps to 15 degrees.
      const r = drag.rects.get(drag.index)!;
      const p = slidePoint(e);
      let deg = (Math.atan2(p.x - (r.x + r.w / 2), -(p.y - (r.y + r.h / 2))) * 180) / Math.PI;
      if (e.shiftKey) deg = Math.round(deg / 15) * 15;
      else if (Math.abs(deg - Math.round(deg / 90) * 90) < 3) deg = Math.round(deg / 90) * 90;
      view.elements[drag.index].rotation = (deg + 360) % 360;
      return;
    }
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
      const turned = (view.elements[drag.index].rotation * Math.PI) / 180;
      if (turned) [dx, dy] = [dx * Math.cos(turned) + dy * Math.sin(turned), -dx * Math.sin(turned) + dy * Math.cos(turned)];
      const sx = snap([r.x + r.w + dx], xs, turned ? 0 : reach);
      const sy = snap([r.y + r.h + dy], ys, turned ? 0 : reach);
      dx += sx.by; dy += sy.by;
      guides = { v: sx.at === null ? [] : [sx.at], h: sy.at === null ? [] : [sy.at] };
      // Lines may be flat: a rectangle keeps a minimum size.
      const flat = view.elements[drag.index].shape === "line" || view.elements[drag.index].shape === "arrow";
      const min = flat ? 0 : 10;
      view.elements[drag.index].rect = { ...r, w: Math.max(min, r.w + dx), h: Math.max(min, r.h + dy) };
    }
  }

  async function endDrag() {
    if (band && view) {
      const b = { x: Math.min(band.x0, band.x1), y: Math.min(band.y0, band.y1), w: Math.abs(band.x1 - band.x0), h: Math.abs(band.y1 - band.y0) };
      band = null;
      if (b.w > 3 || b.h > 3) {
        // Everything the band touches, the topmost first.
        const hit = view.elements
          .filter((el) => el.rect.x < b.x + b.w && el.rect.x + el.rect.w > b.x && el.rect.y < b.y + b.h && el.rect.y + el.rect.h > b.y)
          .map((el) => el.index)
          .reverse();
        selected = hit[0] ?? null;
        also = hit.slice(1);
        banded = true;
      }
      return;
    }
    if (drag?.mode === "rotate" && view) {
      const index = drag.index;
      drag = null;
      await run("Rotated", () => api.setRotation(index, view!.elements[index].rotation));
      return;
    }
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
    // The path tools take the keys while they are active.
    if (pathMode !== null || pathEdit !== null) return;
    if (e.key === "F1") { help = !help; e.preventDefault(); return; }
    // Presenting, from anywhere: F5 from the first slide, Shift+F5 from this one.
    if (e.key === "F5" && view?.slide_count) {
      (document.activeElement as HTMLElement | null)?.blur?.();
      present(!e.shiftKey);
      e.preventDefault();
      return;
    }
    // File shortcuts, as in every editor (also from a text field).
    if ((e.ctrlKey || e.metaKey) && !e.altKey) {
      const k = e.key.toLowerCase();
      const file: Record<string, (() => unknown) | undefined> = {
        n: e.shiftKey ? undefined : () => guarded(() => run("New deck", api.newDeck)),
        o: e.shiftKey ? undefined : () => guarded(openFile),
        s: view?.slide_count ? (e.shiftKey ? saveAs : save) : undefined,
      };
      const f = file[k];
      if (f) { e.preventDefault(); fileMenu = false; f(); return; }
    }
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
      deleteSelection();
      return;
    }
    // Single keys, outside text: T a text box, R a rectangle.
    if (!e.ctrlKey && !e.metaKey && !e.altKey && view?.slide_count && (e.key === "t" || e.key === "r")) {
      if (e.key === "t") run("Added text", api.addText);
      else addShapeOf("rectangle");
      e.preventDefault();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key === "=" || e.key === "+" || e.key === "-" || e.key === "0")) {
      if (e.key === "0") zoom = null;
      else zoomBy(e.key === "-" ? -1 : 1);
      e.preventDefault();
      return;
    }
    // Moving the selection with the arrows: a point, ten with Shift.
    if (!e.ctrlKey && !e.metaKey && !e.altKey && selection.length && view && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) {
      const d = e.shiftKey ? 10 : 1;
      const [dx, dy] = e.key === "ArrowLeft" ? [-d, 0] : e.key === "ArrowRight" ? [d, 0] : e.key === "ArrowUp" ? [0, -d] : [0, d];
      const rects = selection.map((i) => [i, { ...view!.elements[i].rect, x: view!.elements[i].rect.x + dx, y: view!.elements[i].rect.y + dy }] as [number, Rect]);
      enqueue(async () => { await run("Moved", () => api.setRects(rects)); });
      e.preventDefault();
      return;
    }
    if ((e.key === "a" || e.key === "A") && (e.ctrlKey || e.metaKey) && view?.elements.length) {
      selected = view.elements[0].index;
      also = view.elements.slice(1).map((x) => x.index);
      e.preventDefault();
      return;
    }
    if (e.key === "Escape" && selected !== null) {
      selected = null;
      also = [];
      return;
    }
    if (e.key === "Tab" && view?.elements.length) {
      const n = view.elements.length;
      const at = selected === null ? (e.shiftKey ? 0 : -1) : selected;
      selected = (at + (e.shiftKey ? n - 1 : 1) + n) % n;
      also = [];
      e.preventDefault();
      return;
    }
    if ((e.key === "d" || e.key === "D") && (e.ctrlKey || e.metaKey) && selection.length) {
      duplicateSelection();
      e.preventDefault();
      return;
    }
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && e.ctrlKey && view) {
      const to = view.slide + (e.key === "ArrowDown" ? 1 : -1);
      if (to >= 0 && to < view.slide_count) run("Moved slide", () => api.moveSlide(view!.slide, to));
      e.preventDefault();
      return;
    }
    if (e.key === "PageDown" || e.key === "PageUp") {
      if (view) {
        const to = view.slide + (e.key === "PageDown" ? 1 : -1);
        if (to >= 0 && to < view.slide_count) api.goto(to);
      }
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

<svelte:window onkeydown={onKey} onresize={onResize}
               onpointermove={(e) => { thumbMove(e); resizeSlides(e); }}
               onpointerup={() => { thumbUp(); resizingSlides = null; }} onpointercancel={() => { thumbUp(); resizingSlides = null; }} />

<div class="editor" style:grid-template-columns="{slidesOpen ? `${slidesWidth}px` : '36px'} 1fr {panelOpen ? '300px' : '36px'}">
  <header>
    <div class="file-menu">
      <button aria-haspopup="menu" aria-expanded={fileMenu} onclick={() => (fileMenu = !fileMenu)}>File ▾{#if view?.dirty}<span class="dirty" title="Unsaved changes (Ctrl+S saves)"> ●</span>{/if}</button>
      {#if fileMenu}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="menu-backdrop" onclick={() => (fileMenu = false)} oncontextmenu={(e) => { e.preventDefault(); fileMenu = false; }}></div>
        <div class="context-menu file" role="menu">
          <button role="menuitem" onclick={() => fileAction(() => guarded(() => run("New deck", api.newDeck)))}>New <kbd>Ctrl+N</kbd></button>
          <button role="menuitem" title="A PowerPoint file (.pptx, .pptm), a single-file deck (.deckz) or a deck.json" onclick={() => fileAction(() => guarded(openFile))}>Open file… <kbd>Ctrl+O</kbd></button>
          <button role="menuitem" onclick={() => fileAction(() => guarded(openFolder))}>Open .deck folder…</button>
          <hr />
          <button role="menuitem" disabled={!view?.slide_count} onclick={() => fileAction(save)}>Save <kbd>Ctrl+S</kbd></button>
          <button role="menuitem" disabled={!view?.slide_count} onclick={() => fileAction(saveAs)}>Save as… <kbd>Ctrl+Shift+S</kbd></button>
          <hr />
          <button role="menuitem" disabled={!view?.dir} title="Copy slides from another deck, with their images, layouts and themes" onclick={() => fileAction(pickDeck)}>Insert slides from deck…</button>
          <hr />
          <button role="menuitem" disabled={!view?.slide_count} onclick={() => fileAction(exportPptx)}>Export PowerPoint…</button>
          <button role="menuitem" disabled={!view?.slide_count} onclick={() => fileAction(() => (pdfDialog = true))}>Export PDF…</button>
        </div>
      {/if}
    </div>
    <span class="spacer"></span>
    <button class="end" title="Undo (Ctrl+Z)" disabled={!view?.can_undo} onclick={() => undoRedo(false)}>↶</button>
    <button class="end" title="Redo (Ctrl+Y)" disabled={!view?.can_redo} onclick={() => undoRedo(true)}>↷</button>
    <button class="end" title="Commands and keys (F1)" onclick={() => (help = !help)}>?</button>
    {#if view?.slide_count}
      <div class="views" role="group" aria-label="View">
        <button class:on={!editingTemplate} title="Edit the slides" onclick={() => { if (editingTemplate) exitDesign(); }}>Slides</button>
        <button class:on={editingTemplate} title="Edit the themes and layouts: backgrounds, shared elements, placeholders, fonts and colors" onclick={() => { if (!editingTemplate) enterDesign(); }}>Design</button>
      </div>
    {/if}
    {#if view?.editing_theme !== null && view?.editing_theme !== undefined}
      <span class="banner">Theme
        <input class="layout-name" value={view.editing_theme} onchange={(e) => run("Theme renamed", () => api.renameTheme(e.currentTarget.value))} />
      </span>
      <button onclick={() => run("Added text", api.addText)}>+ Text</button>
      <MenuButton label="+ Shape" title="Add a shape, or draw a path" items={SHAPES} onpick={addShapeOf} />
      <button class="primary" onclick={exitDesign}>Close design view</button>
    {:else if view?.editing_layout !== null && view?.editing_layout !== undefined}
      <span class="banner">Layout
        <input class="layout-name" value={view.editing_layout} onchange={(e) => run("Layout renamed", () => api.renameLayout(e.currentTarget.value))} />
      </span>
      <button onclick={() => run("Added text", api.addText)}>+ Text</button>
      <MenuButton label="+ Shape" title="Add a shape, or draw a path" items={SHAPES} onpick={addShapeOf} />
      <button onclick={() => enqueue(() => run("Added table", () => api.addTable(3, 3)).then(selectAdded))} title="A table of 3 by 3 cells with a header row; edit it in the panel">+ Table</button>
      <MenuButton label="+ Placeholder" title="A slot new slides start from" items={[{ value: "title", label: "Title" }, { value: "subtitle", label: "Subtitle" }, { value: "body", label: "Body" }]}
                  onpick={(k) => run("Added placeholder", () => api.addPlaceholder(k as "title" | "subtitle" | "body"))} />
      <button class="primary" onclick={exitDesign}>Close design view</button>
    {:else if view?.slide_count}
      <button onclick={() => run("Added text", api.addText)}>+ Text</button>
      <MenuButton label="+ Shape" title="Add a shape, or draw a path" items={SHAPES} onpick={addShapeOf} />
      <button onclick={() => enqueue(() => run("Added table", () => api.addTable(3, 3)).then(selectAdded))} title="A table of 3 by 3 cells with a header row; edit it in the panel">+ Table</button>
      <button onclick={addMedia} title="A video or an animated GIF; images and clips can also be pasted or dropped">+ Video…</button>
      <button onclick={() => run("Added figure", api.addFigure)} title="A Typst figure: CeTZ drawings and plots, edited as source in the panel">+ Figure</button>
      {#if selection.length >= 2}<button onclick={groupSelection} title="Group the selected elements (Ctrl+G)">Group</button>{/if}
      {#if sel?.kind === "group"}<button onclick={ungroupSelection} title="Ungroup (Ctrl+Shift+G)">Ungroup</button>{/if}
      {#if view.presenting}
        <button class="primary" onclick={() => api.stopPresentation()}>Stop (Esc)</button>
      {:else}
        <MenuButton label="Present" primary align="right" title="Start the presentation"
                    items={[{ value: "here", label: "From this slide", key: "Shift+F5" }, { value: "start", label: "From the first slide", key: "F5" }]}
                    onpick={(k) => present(k === "start")} />
      {/if}
    {/if}
  </header>

  {#if view && view.slide_count}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    {#if slidesOpen}
      <!-- On the pane's edge, outside its scrolling: it stays put. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="pane-resizer" title="Drag to resize the slides" onpointerdown={startResizeSlides}></div>
    {/if}
    {#if editingTemplate}
    <aside class="slides design" class:collapsed={!slidesOpen} oncontextmenu={(e) => openTreeMenu(e, null)}>
      <button class="pane-toggle" title={slidesOpen ? "Collapse the pane" : "Show the themes and layouts"} onclick={() => (slidesOpen = !slidesOpen)}>{slidesOpen ? "«" : "»"}</button>
      {#if slidesOpen}
        {#each tree as item (item.kind + ":" + item.name)}
          {@const current = item.kind === "theme" ? view.editing_theme === item.name : view.editing_layout === item.name}
          <button class="design-item {item.kind}" class:current class:loose={item.kind === "layout" && !item.theme}
                  title={item.kind === "theme" ? `Theme “${item.name}”: background, shared elements, placeholder styles, fonts and colors` : `Layout “${item.name}”${item.used ? `, used by ${item.used} slide${item.used > 1 ? "s" : ""}` : ", not used"}`}
                  onclick={() => openDesignItem(item)} oncontextmenu={(e) => openTreeMenu(e, item)}
                  ondblclick={() => { renamingItem = { kind: item.kind, name: item.name, value: item.name }; queueMicrotask(() => { const i = document.querySelector<HTMLInputElement>(".rename-dialog input"); i?.focus(); i?.select(); }); }}>
            <SlideView svg={item.svg} width={view.width} height={view.height} />
            <span class="design-name">{item.kind === "theme" ? "◆ " : ""}{item.name}{#if item.kind === "layout" && item.used}<span class="used">{item.used}</span>{/if}</span>
          </button>
        {/each}
        <p class="muted pane-hint">Right-click for new layouts and themes.</p>
      {/if}
    </aside>
    {:else}
    <aside class="slides" class:collapsed={!slidesOpen} oncontextmenu={openGapMenu}>
      <button class="pane-toggle" title={slidesOpen ? "Collapse the slides: more room for the slide" : "Show the slides"} onclick={() => (slidesOpen = !slidesOpen)}>{slidesOpen ? "«" : "»"}</button>
      {#if slidesOpen}
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
        <div class="stack-item" data-index={i} class:drop-before={dropAt === i} class:drop-after={dropAt === i + 1 && i === thumbnails.length - 1} class:dragging={dragging === i}
             onpointerdown={(e) => thumbDown(e, i)}>
          <button class="thumb" class:current={i === view.slide} class:picked={picked.includes(i)} onclick={(e) => clickThumb(e, i)} oncontextmenu={(e) => openSlideMenu(e, i)}>
            <span class="num">{i + 1}{#if view.starts[i]?.clicks}<span class="anim" title="{view.starts[i].clicks} click{view.starts[i].clicks > 1 ? 's' : ''} of animation">★</span>{/if}</span>
            <SlideView svg={thumb} width={view.width} height={view.height} />
          </button>
        </div>
      {/each}
      {/if}
    </aside>
    {/if}

    <main>
      <!-- The toolbars keep their place whether or not something is selected, so the canvas never jumps under the pointer. -->
      <div class="bars">
      {#if !sel}<div class="bars-hint">Select an element to style it; drag on the empty slide to select several.</div>{/if}
      {#if sel && sel.kind !== "image" && sel.kind !== "placeholder"}
        <StyleBar element={sel} theme={view.theme}
                  range={editing && editing.index === sel.index ? { from: Math.min(editing.caret, editing.anchor), to: Math.max(editing.caret, editing.anchor) } : null}
                  onrun={(label, f) => enqueue(() => run(label, f).then(() => refreshLayout()))} />
      {/if}
      {#if selection.length && !editing}
        {#snippet arrange()}
          <ArrangeBar elements={selection.map((i) => view!.elements[i]).filter((e) => e && e.kind !== "placeholder")}
                      onrun={(label, f) => enqueue(async () => { await run(label, f); })}
                      onmoved={(ix) => { if (ix.length) { selected = ix[0]; also = ix.slice(1); } }} />
        {/snippet}
        {#if sel && (sel.kind === "text" || sel.kind === "shape")}
          <!-- With the full text tools in the row, the arrange tools fold into a button. -->
          <div class="fold-tools">
            <button class:on={arrangeOpen} aria-expanded={arrangeOpen} title="Align, distribute, mirror, order, stroke" onclick={() => (arrangeOpen = !arrangeOpen)}>Arrange ▾</button>
          </div>
        {:else}
          {@render arrange()}
        {/if}
      {/if}
      </div>
      {#if arrangeOpen && sel && (sel.kind === "text" || sel.kind === "shape") && selection.length && !editing}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="menu-backdrop" onclick={() => (arrangeOpen = false)}></div>
        <div class="arrange-pop">
          <ArrangeBar elements={selection.map((i) => view!.elements[i]).filter((e) => e && e.kind !== "placeholder")}
                      onrun={(label, f) => enqueue(async () => { await run(label, f); })}
                      onmoved={(ix) => { if (ix.length) { selected = ix[0]; also = ix.slice(1); } }} />
        </div>
      {/if}
      <div class="canvas-wrap" class:zoomed={zoom !== null} bind:clientWidth={wrapW} bind:clientHeight={wrapH}>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="canvas" class:drop-hover={dropHover} bind:this={canvas} style="aspect-ratio: {view.width} / {view.height}; width: {zoom !== null ? view.width * zoom : fitWidth}px"
             onpointerdown={startBand} onpointermove={moveDrag} onpointerup={endDrag} onpointercancel={endDrag}
             onclick={(e) => { if (banded) { banded = false; return; } if ((e.target as Element).closest(".hit, .handle, .rotate, .text-editing")) return; selected = null; also = []; editing = null; }}>
          <SlideView svg={view.svg} width={view.width} height={view.height} dir={view.dir} />
          <svg class="overlay" viewBox="0 0 {view.width} {view.height}" preserveAspectRatio="none">
            {#each view.elements as el (el.index)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              {@const turn = el.rotation ? `rotate(${el.rotation} ${el.rect.x + el.rect.w / 2} ${el.rect.y + el.rect.h / 2})` : undefined}
              {#if el.shape === "line" || el.shape === "arrow"}
                <!-- A line's box can be flat: it is grabbed along the line itself. -->
                <line class="hit thin" class:selected={selection.includes(el.index)} transform={turn}
                      x1={el.rect.x} y1={el.flip ? el.rect.y + el.rect.h : el.rect.y}
                      x2={el.rect.x + el.rect.w} y2={el.flip ? el.rect.y : el.rect.y + el.rect.h}
                      onpointerdown={(e) => startDrag(e, el.index, "move")} oncontextmenu={(e) => openElementMenu(e, el.index)} />
              {:else}
                <rect class="hit" class:selected={selection.includes(el.index)} transform={turn}
                      x={el.rect.x} y={el.rect.y} width={el.rect.w} height={el.rect.h}
                      onpointerdown={(e) => startDrag(e, el.index, "move")} oncontextmenu={(e) => openElementMenu(e, el.index)}
                      ondblclick={(e) => { e.stopPropagation(); startEdit(el.index, slidePoint(e)); }} />
              {/if}
              {#if el.index === selected}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <rect class="handle" x={el.rect.x + el.rect.w - 6} y={el.rect.y + el.rect.h - 6} width="12" height="12" transform={turn}
                      onpointerdown={(e) => startDrag(e, el.index, "resize")} />
                {#if el.kind !== "placeholder" && also.length === 0}
                  <g transform={turn}>
                    <line class="rotate-stem" x1={el.rect.x + el.rect.w / 2} y1={el.rect.y} x2={el.rect.x + el.rect.w / 2} y2={el.rect.y - 22} />
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <circle class="rotate" cx={el.rect.x + el.rect.w / 2} cy={el.rect.y - 22} r="7"
                            onpointerdown={(e) => startDrag(e, el.index, "rotate")}
                            ondblclick={(e) => { e.stopPropagation(); run("Rotation reset", () => api.setRotation(el.index, 0)); }}>
                      <title>Drag to rotate (Shift: steps of 15°); double-click to reset{el.rotation ? ` · ${Math.round(el.rotation)}°` : ""}</title>
                    </circle>
                  </g>
                {/if}
              {/if}
            {/each}
            {#if band}
              <rect class="band" x={Math.min(band.x0, band.x1)} y={Math.min(band.y0, band.y1)} width={Math.abs(band.x1 - band.x0)} height={Math.abs(band.y1 - band.y0)} />
            {/if}
            {#each guides.v as x}<line class="guide" x1={x} y1="0" x2={x} y2={view.height} />{/each}
            {#each guides.h as y}<line class="guide" x1="0" y1={y} x2={view.width} y2={y} />{/each}
          </svg>
          {#if pathMode !== null}
            <PathTool mode={pathMode} width={view.width} height={view.height} unit={unit()} toSlide={slidePoint}
                      oncommit={pathDrawn} ondone={() => { pathMode = null; status = ""; }} />
          {:else if pathEdit !== null && view.elements[pathEdit]?.path}
            {@const pe = view.elements[pathEdit]}
            <PathTool mode="edit" width={view.width} height={view.height} unit={unit()} toSlide={slidePoint}
                      edit={{ nodes: pe.path!.nodes, closed: pe.path!.closed, rotation: pe.rotation, rect: pe.rect }}
                      oncommit={pathEdited} ondone={() => { pathEdit = null; status = ""; }} />
          {/if}
          {#if phRename && view.elements[phRename.index]}
            {@const r = view.elements[phRename.index].rect}
            <input class="ph-rename" bind:value={phRename.value}
                   style="left: {(r.x / view.width) * 100}%; top: {(r.y / view.height) * 100}%; width: {Math.max(18, (r.w / view.width) * 100)}%"
                   onkeydown={(e) => { if (e.key === "Enter") commitPhRename(); if (e.key === "Escape") phRename = null; e.stopPropagation(); }}
                   onblur={commitPhRename} />
          {/if}
          {#if editing && view.elements[editing.index]}
            {@const r = editing.layout.rect ?? view.elements[editing.index].rect}
            <!-- Clamped: a throw here would stop the whole canvas from updating. -->
            {@const c = editing.layout.chars[Math.min(editing.caret, editing.layout.chars.length - 1)] ?? { x: r.x, top: r.y, bottom: r.y + 20 }}
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
        {#if editingTemplate}{view.editing_theme !== null ? `Theme “${view.editing_theme}”` : `Layout “${view.editing_layout}”`}{:else}Slide {view.slide + 1} / {view.slide_count}{/if}
        {#if !editingTemplate && (view.last_step > 0 || view.step > 0)}
          <span class="clicks" title="The slide at a click of its animation, or with every element shown for editing">
            <button class:on={view.all} title="Every element shown, for editing" onclick={() => api.goto(view!.slide)}>All</button>
            <button title="Previous click" disabled={view.all || view.step === 0} onclick={() => api.goto(view!.slide, Math.max(0, view!.step - 1))}>◀</button>
            <span class="at">{view.all ? `${view.last_step} click${view.last_step > 1 ? "s" : ""}` : view.step === 0 ? "start" : `click ${view.step} / ${view.last_step}`}</span>
            <button title="Next click (from All: the slide as it opens)" disabled={!view.all && view.step >= view.last_step} onclick={() => api.goto(view!.slide, view!.all ? 0 : view!.step + 1)}>▶</button>
          </span>
        {/if}
        {#if !editingTemplate}
          <span class="spacer"></span>
          <button class="fold" aria-expanded={notesOpen} onclick={() => (notesOpen = !notesOpen)}>Notes {notesOpen ? "▾" : "▸"}</button>
        {/if}
      </div>
      {#if !editingTemplate && notesOpen}
        <textarea class="notes" rows="3" placeholder="Speaker notes for this slide: shown in the presenter view and the handout"
                  value={view.notes} onchange={(e) => api.setNotes(e.currentTarget.value)}></textarea>
      {/if}
    </main>

    <aside class="props" class:collapsed={!panelOpen} onfocusin={trackVarTarget}>
      <div class="tabs" class:vertical={!panelOpen}>
        <button class:on={panelOpen && panelTab === "element"} title="The selected element: name, position, its own settings" onclick={() => showTab("element")}>{panelOpen ? "Element" : "◻"}</button>
        <button class:on={panelOpen && panelTab === "slide"} title="The slide: section, layout, theme, fonts and colors, macros" onclick={() => showTab("slide")}>{panelOpen ? (view.editing_theme !== null ? "Theme" : view.editing_layout !== null ? "Layout" : "Slide") : "☰"}</button>
        {#if !editingTemplate}
          <button class:on={panelOpen && panelTab === "anim"} title="Animations: the timeline of the slide's clicks" onclick={() => showTab("anim")}>{panelOpen ? "Animations" : "▶"}</button>
        {/if}
        {#if panelOpen}<button class="collapse" title="Collapse the panel: more room for the slide" onclick={() => (panelOpen = false)}>⟩</button>{/if}
      </div>
      {#if !panelOpen}
        <!-- collapsed: only the tabs -->
      {:else if panelTab === "anim" && !editingTemplate}
        {#if editingTemplate}
          <p class="muted">Layouts and themes have no animation: steps belong to slides.</p>
        {:else}
          <h3>Timeline</h3>
          <Timeline elements={view.elements} last={view.last_step} current={view.step} {selected}
                    onselect={(i) => { selected = i; also = []; }}
                    ongoto={(step) => api.goto(view!.slide, step)}
                    onmove={(element, step, to, insert, position) => run(insert ? "Step on a click of its own" : "Step moved", () => api.moveStep(element, step, to, insert, position))}
                    ontrigger={(element, step, mode) => run("Start of the effect", () => api.setTrigger(element, step, mode))} />
          {#if sel && sel.kind !== "placeholder"}
            <h3>Steps of {sel.label}</h3>
            <Steps element={sel} current={view.step} last={view.last_step} firstOfClick={(st) => firstOfClick(sel!.index, st)}
                   onrun={(label, f) => run(label, f)} ongoto={(step) => api.goto(view!.slide, step)} />
          {:else}
            <p class="muted">Select an element to add or edit its steps: appear or disappear on a click, move, grow, turn, change color.</p>
          {/if}
        {/if}
      {:else if panelTab === "element"}
      {#if sel}
        <h3>{sel.kind === "math" ? "Formula" : sel.kind === "outline" ? "Plan" : sel.kind === "media" ? "Video" : sel.shape === "path" ? "Path" : sel.kind}</h3>
        {#if sel.kind !== "placeholder"}
          <label class="name">name
            <input type="text" value={sel.name} placeholder={sel.label}
                   title="Shown in the animation timeline, and kept by PowerPoint in its selection pane"
                   onchange={(e) => { const i = sel!.index; const v = e.currentTarget.value; run(v ? "Named" : "Name removed", () => api.setName(i, v)); }}
                   onkeydown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }} />
          </label>
        {/if}
<div class="xywh">
        <label>x <input type="number" value={Math.round(sel.rect.x)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, x: +e.currentTarget.value })} /></label>
        <label>y <input type="number" value={Math.round(sel.rect.y)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, y: +e.currentTarget.value })} /></label>
        <label>w <input type="number" value={Math.round(sel.rect.w)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, w: +e.currentTarget.value })} /></label>
        <label>h <input type="number" value={Math.round(sel.rect.h)} onchange={(e) => api.setRect(sel!.index, { ...sel!.rect, h: +e.currentTarget.value })} /></label>
        </div>
        {#if sel.path}
          {@const path = sel.path}
          <h3>Path</h3>
          <label class="check"><input type="checkbox" checked={path.closed}
                 onchange={(e) => { const c = e.currentTarget.checked; const i = sel!.index; run(c ? "Path closed" : "Path opened", () => api.setPath(i, path.nodes, c)); }} />
            closed: the last point joins the first, and the shape is filled</label>
          <div class="row"><button onclick={() => startEdit(sel!.index)}>Edit points</button></div>
          <p class="muted" title="Drag points and handles; double-click a point for a corner or a curve; Delete removes it.">{path.nodes.length} points · double-click the path to edit them.</p>
        {/if}
        {#if sel.kind === "media" && sel.media}
          <h3>Playback</h3>
          <label><input type="radio" name="play" checked={sel.media.play === "on_appear"} onchange={() => run("Play on appearance", () => api.styleElement(sel!.index, { play: "appear" }))} /> play when it appears</label>
          <label><input type="radio" name="play" checked={sel.media.play === "button"} onchange={() => run("Play on click", () => api.styleElement(sel!.index, { play: "button" }))} /> play on its button</label>
          <label><input type="checkbox" checked={sel.media.looping} onchange={(e) => run("Loop", () => api.styleElement(sel!.index, { looping: e.currentTarget.checked }))} /> loop (else play once and stop)</label>
          {#if !sel.text.toLowerCase().endsWith(".gif")}
            <div class="row">
              <label title="The frame to keep as the poster, in seconds">poster at <input type="number" min="0" step="0.5" bind:value={posterAt} style="width: 64px" /> s</label>
              <button onclick={() => capturePoster(sel!.index, sel!.text)} title="Keep that frame as the still shown before playback, in thumbnails, PDF and PowerPoint">Use as poster</button>
              {#if sel.media.poster}<button onclick={() => run("Poster removed", () => api.setPoster(sel!.index, null))}>Remove</button>{/if}
            </div>
            <label><input type="checkbox" checked={sel.media.sound} onchange={(e) => run("Sound", () => api.styleElement(sel!.index, { sound: e.currentTarget.checked }))} /> sound</label>
          {/if}
          <p class="muted" title="The editor shows the first frame; thumbnails, PDF and PowerPoint show the poster.">{sel.text}{sel.media.duration ? ` · ${sel.media.duration.toFixed(1)} s` : ""} · plays in the presentation.</p>
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
          <p class="muted" title="Duplicate this slide and drag a copy to the start of each section: each copy highlights its own.">The sections, with this slide's in bold.</p>
        {:else if sel.kind === "placeholder"}
          <p class="muted" title="Slides fill this slot by name; title and subtitle map to PowerPoint's own placeholders.">Placeholder “{sel.text}”: double-click its box to rename it.</p>
          {#if sel.placeholder}
            {@const idx = sel.index}
            <label>prefix
              <input type="text" class="vars-target" value={sel.placeholder.prefix ?? ""} placeholder="e.g. Part {"{{section}}"} · "
                     title="Shown before each slide's own text in this placeholder"
                     onchange={(e) => run("Placeholder prefix", () => api.stylePlaceholder(idx, { prefix: e.currentTarget.value }))} />
            </label>
            <Variables oninsert={insertVariable} />
          {/if}
          {#if sel.placeholder}
            {@const ph = sel.placeholder}
            {@const idx = sel.index}
            <h3>Default text style</h3>
            <PlaceholderFields {ph} fonts={view.theme.fonts} themeSize={view.theme.effective.size} themeColor={view.theme.effective.color}
              inTheme={!!view.layout_meta?.parent}
              onchange={(change) => run("Placeholder style", () => api.stylePlaceholder(idx, change))}
              onreset={(prop) => run("Back to the theme", () => api.resetToTheme(idx, prop))}
              onpush={(prop) => run("Made the theme's", () => api.pushToTheme(idx, prop))} />
            <p class="muted">New slides made from this layout start with this style; existing slides keep theirs.</p>
          {/if}
        {:else if sel.kind === "table" && sel.table}
          <TablePanel element={sel} onrun={(label, f) => enqueue(async () => { await run(label, f); })} />
        {:else if sel.kind !== "image" && sel.kind !== "media"}
          <label>{sel.kind === "math" ? "formula" : sel.kind === "figure" ? "Typst source" : "text"}
            <textarea rows={sel.kind === "figure" ? 12 : 6} class:code={sel.kind === "figure" || sel.kind === "math"} value={sel.text} onchange={(e) => api.setText(sel!.index, e.currentTarget.value)}></textarea>
          </label>
          {#if sel.kind === "figure"}
            <p class="muted" title="Bundled: @preview/cetz:0.5.2 and @preview/cetz-plot:0.1.4 (plot.add(points), plot.add-bar, draw.line…). The page is as wide as the box and scaled to fit it; a Python macro can generate it from data.">Typst markup; CeTZ plots and drawings are bundled.</p>
          {/if}
        {/if}
        {#if sel.kind !== "placeholder" && !editingTemplate}
          <h3>Animation</h3>
          <p class="muted">{sel.last_step ? `${sel.steps.filter((x) => x.at > 0).length} step(s)` : "None"} · <button class="link" onclick={() => { panelTab = "anim"; }}>open the timeline</button></p>
        {/if}
      {:else}
        <p class="muted">Nothing selected. Click an element, or drag on the slide to select several. <button class="link" onclick={() => (help = true)}>Keys…</button></p>
      {/if}
      {:else}
      {#if !editingTemplate}
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
      {#if view.editing_theme !== null}
        <h3>Placeholder styles</h3>
        <p class="muted">The style each kind of placeholder starts with in this theme's layouts, unless a layout sets its own. Body styles apply to body1, body2…</p>
        {#each view.theme_placeholders as tph (tph.name)}
          <details class="kind">
            <summary>{tph.name}</summary>
            <PlaceholderFields ph={tph} fonts={view.theme.fonts} themeSize={view.theme.effective.size} themeColor={view.theme.effective.color}
              onchange={(change) => run(`Theme ${tph.name} style`, () => api.styleThemePlaceholder(tph.name, change))} />
          </details>
        {/each}
        <h3>Variables</h3>
        <Variables oninsert={insertVariable} />
        <h3>Layouts of this theme</h3>
        <div class="row">
          {#each view.layout_groups.find((g) => g.theme === view!.editing_theme)?.layouts ?? [] as name}
            <button onclick={() => run("Editing layout", () => api.editLayout(name))}>{name}</button>
          {:else}
            <span class="muted">None yet: "+ New layout" on a slide of this theme.</span>
          {/each}
        </div>
      {:else if view.editing_layout !== null && view.layout_meta}
        {@const meta = view.layout_meta}
        <h3>Variables</h3>
        <Variables oninsert={insertVariable} />
        <h3>Theme</h3>
        <label>theme
          <select value={meta.parent ?? ""} onchange={(e) => run("Layout theme", () => api.setLayoutTheme(e.currentTarget.value || null))}>
            <option value="">none</option>
            {#each meta.themes as t}<option value={t}>{t}</option>{/each}
          </select>
        </label>
        {#if meta.parent}
          <label class="check"><input type="checkbox" checked={meta.hide_theme_elements} onchange={(e) => run("Theme elements", () => api.hideThemeElements(e.currentTarget.checked))} /> hide the theme's elements (logo, footer…)</label>
          {#if meta.overrides.length}
            <p class="muted">Changes the theme's: {meta.overrides.join("; ")}.</p>
            <button onclick={() => run("Back to the theme", () => api.resetToTheme())}>Reset to theme</button>
          {:else}
            <p class="muted">Follows its theme in everything but placeholder positions.</p>
          {/if}
          <div class="row"><button onclick={() => run("Editing theme", () => api.editTheme(meta.parent))}>Edit theme “{meta.parent}”</button></div>
        {/if}
        <h3>Layout</h3>
        <p class="muted" title="Variables filled per slide in texts: {"{{section}}"}, {"{{subsection}}"}, {"{{slide}}"}, {"{{slides}}"}, {"{{title}}"}, {"{{deck}}"}, {"{{date}}"}">Own elements over the theme's (dimmed); dashed boxes are the placeholders new slides start from. Texts can hold variables such as <code>{"{{slide}}"}</code>.</p>
      {:else}
        <h3>Layout</h3>
        {#if view.layouts.length}
          <select value={view.layout ?? ""} onchange={(e) => run("Layout set", () => api.setLayout(e.currentTarget.value || null))}>
            <option value="">none</option>
            {#each view.layout_groups as g}
              {#if g.theme}
                <optgroup label={g.theme}>{#each g.layouts as name}<option value={name}>{name}</option>{/each}</optgroup>
              {:else}
                {#each g.layouts as name}<option value={name}>{name}</option>{/each}
              {/if}
            {/each}
          </select>
          <p class="muted">"New slide" (right-click a thumbnail) starts from this layout's placeholders.</p>
        {:else}
          <p class="muted">No layouts yet: make some in the design view.</p>
        {/if}
        <div class="row">
          <button title="Edit the themes and layouts: backgrounds, shared elements, placeholders, fonts and colors" onclick={enterDesign}>Design view…</button>
        </div>
        <p class="muted">Backgrounds, logos, placeholders, fonts and colors are edited in the design view.</p>
      {/if}
      {#if editingTemplate}
      <h3>Fonts and colors{view.editing_theme !== null ? " of this theme" : view.editing_layout !== null ? " of this layout" : view.theme.of_theme ? ` of theme “${view.theme.of_theme}”` : ""}</h3>
      <p class="muted">
        {#if view.editing_theme !== null}
          The default font, size and colors of the slides of this theme's layouts.
        {:else if view.editing_layout !== null}
          Blank fields take the theme's; a value set here is this layout's own (⌀ goes back to the theme's).
        {:else}
          Text that sets none of its own takes these. They belong to the slide's theme: a change here
          restyles every slide of that theme. Fonts in the deck's <code>fonts/</code> folder are listed here.
        {/if}
      </p>
      <div class="theme">
        <label>font
          <select value={view.theme.own.font ?? ""} onchange={(e) => run("Theme font", () => api.setTheme({ font: e.currentTarget.value }))}>
            <option value="">{view.editing_layout !== null && view.theme.effective.font ? `theme (${view.theme.effective.font})` : "DejaVu Sans"}</option>
            {#each view.theme.fonts as f}<option value={f}>{f}</option>{/each}
          </select>
        </label>
        <label>size
          <input type="number" min="0" max="200" step="1" value={view.theme.own.size ?? ""} placeholder={String(view.theme.effective.size ?? 18)}
                 onchange={(e) => run("Theme size", () => api.setTheme({ size: +e.currentTarget.value || 0 }))} />
        </label>
        <label>text color
          <input type="color" value={view.theme.effective.color ?? "#000000"} onchange={(e) => run("Theme text color", () => api.setTheme({ color: e.currentTarget.value }))} />
          <button title="Back to the inherited color" onclick={() => run("Theme text color reset", () => api.setTheme({ color: "" }))}>⌀</button>
        </label>
        <label>accent
          <input type="color" value={view.theme.effective.accent ?? "#000000"} onchange={(e) => run("Theme accent", () => api.setTheme({ accent: e.currentTarget.value }))} />
          <button title="Back to the inherited accent" onclick={() => run("Theme accent reset", () => api.setTheme({ accent: "" }))}>⌀</button>
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
            <span class="muted">(theme's)</span>
          {/if}
        </div>
      </div>
      {/if}
      {#if !editingTemplate}
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
      {/if}
      {#if view.warnings.length}
        <h3>Import warnings</h3>
        <ul class="muted">{#each view.warnings as w}<li>{w}</li>{/each}</ul>
      {/if}
      {/if}
    </aside>
  {:else}
    <div class="empty">
      <img class="logo" src={lockup} alt="decker: Efficient Cross-platform Keynote Editor and Renderer" />
      <div class="row">
        <button class="primary" onclick={() => run("New deck", api.newDeck)}>New deck</button>
        <button onclick={openFile} title="A PowerPoint file (.pptx, .pptm), a single-file deck (.deckz) or a deck.json">Open file…</button>
        <button onclick={openFolder}>Open .deck folder…</button>
      </div>
      <p class="muted">Ctrl+N · Ctrl+O · F1 for the commands and keys</p>
    </div>
  {/if}

  <footer>
    <span class="status">{status}</span>
    {#if view?.slide_count && !editing}
      <span class="zoom">
        <button title="Zoom out (Ctrl+-)" onclick={() => zoomBy(-1)}>−</button>
        <button class:on={zoom === null} title="Fit the slide in the window (Ctrl+0)" onclick={() => (zoom = null)}>Fit</button>
        <button class:on={zoom === 1} title="Actual size: one point per pixel" onclick={() => (zoom = 1)}>100%</button>
        <button title="Zoom in (Ctrl++)" onclick={() => zoomBy(1)}>+</button>
        <span class="pct">{zoomShown}%</span>
      </span>
    {/if}
  </footer>
  {#if help}<Help onclose={() => (help = false)} />{/if}

  {#if gapMenu && view}
    {@const g = gapMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="menu-backdrop" onclick={() => (gapMenu = null)} oncontextmenu={(e) => { e.preventDefault(); gapMenu = null; }}></div>
    <div class="context-menu" role="menu" style="left: {g.x}px; top: {g.y}px">
      <div class="menu-title">{g.at >= view.slide_count ? "At the end" : `Before slide ${g.at + 1}`}</div>
      <button role="menuitem" onclick={() => onGap("slide")}>New slide</button>
      <button role="menuitem" title="A slide listing the sections, the current one highlighted" onclick={() => onGap("plan")}>New plan slide</button>
      <button role="menuitem" onclick={() => onGap("section")}>New section</button>
      <button role="menuitem" onclick={() => onGap("subsection")}>New subsection</button>
    </div>
  {/if}

  {#if treeMenu && view}
    {@const m = treeMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="menu-backdrop" onclick={() => (treeMenu = null)} oncontextmenu={(e) => { e.preventDefault(); treeMenu = null; }}></div>
    <div class="context-menu" role="menu" style="left: {m.x}px; top: {m.y}px">
      {#if m.item?.kind === "theme" || (m.item?.kind === "layout" && m.item.theme)}
        {@const theme = m.item.kind === "theme" ? m.item.name : m.item.theme}
        <div class="menu-title">New layout in “{theme}”</div>
        {#each PRESETS as [kind, label]}
          <button role="menuitem" onclick={() => { const t = theme; treeAction(() => run("New layout", () => api.newLayout(kind, t))); }}>{label}</button>
        {/each}
        <hr />
      {/if}
      {#if m.item}
        {@const it = m.item}
        <button role="menuitem" onclick={() => { const { kind, name } = it; treeAction(() => {
          renamingItem = { kind, name, value: name };
          queueMicrotask(() => { const i = document.querySelector<HTMLInputElement>(".rename-dialog input"); i?.focus(); i?.select(); });
        }); }}>Rename “{it.name}”…</button>
      {/if}
      {#if m.item?.kind === "theme"}
        <button role="menuitem" onclick={() => { const n = m.item!.name; treeAction(async () => { await api.editTheme(n); run("Theme deleted", api.deleteTheme); }); }}>Delete theme “{m.item.name}”</button>
      {:else if m.item?.kind === "layout"}
        <button role="menuitem" disabled={!view.dir} onclick={() => { const n = m.item!.name; treeAction(async () => { await api.editLayout(n); saveLayoutAs(); }); }}>Save layout to a file…</button>
        <button role="menuitem" onclick={() => { const n = m.item!.name; treeAction(async () => { await api.editLayout(n); run("Layout deleted", api.deleteLayout); }); }}>Delete layout “{m.item.name}”</button>
      {/if}
      {#if !m.item}<div class="menu-title">Design</div>{/if}
      <button role="menuitem" onclick={() => treeAction(() => run("New theme", api.newTheme))}>New theme</button>
      <button role="menuitem" disabled={!view.dir} onclick={() => treeAction(loadLayoutFrom)}>Load a layout from a file…</button>
    </div>
  {/if}

  {#if elementMenu && view && sel}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="menu-backdrop" onclick={() => (elementMenu = null)} oncontextmenu={(e) => { e.preventDefault(); elementMenu = null; }}></div>
    <div class="context-menu" role="menu" style="left: {elementMenu.x}px; top: {elementMenu.y}px">
      <div class="menu-title">{selection.length > 1 ? `${selection.length} elements` : sel.label}</div>
      <button role="menuitem" onclick={() => onElement(() => copySelection(true))}>Cut <kbd>Ctrl+X</kbd></button>
      <button role="menuitem" onclick={() => onElement(() => copySelection(false))}>Copy <kbd>Ctrl+C</kbd></button>
      <button role="menuitem" onclick={() => onElement(pasteClipboard)}>Paste <kbd>Ctrl+V</kbd></button>
      <button role="menuitem" onclick={() => onElement(duplicateSelection)}>Duplicate <kbd>Ctrl+D</kbd></button>
      <button role="menuitem" onclick={() => onElement(deleteSelection)}>Delete <kbd>Del</kbd></button>
      <hr />
      <button role="menuitem" onclick={() => onElement(() => orderSelection("front"))}>Bring to front</button>
      <button role="menuitem" onclick={() => onElement(() => orderSelection("forward"))}>Bring forward</button>
      <button role="menuitem" onclick={() => onElement(() => orderSelection("backward"))}>Send backward</button>
      <button role="menuitem" onclick={() => onElement(() => orderSelection("back"))}>Send to back</button>
      {#if selection.length >= 2}<button role="menuitem" onclick={() => onElement(groupSelection)}>Group <kbd>Ctrl+G</kbd></button>{/if}
      {#if sel.kind === "group"}<button role="menuitem" onclick={() => onElement(ungroupSelection)}>Ungroup <kbd>Ctrl+Shift+G</kbd></button>{/if}
      <hr />
      <button role="menuitem" onclick={() => onElement(renameSelection)}>Rename…</button>
      {#if selection.length === 1 && !editingTemplate}
        <button role="menuitem" onclick={() => onElement(() => animateSelection(true))}>Appear on next click</button>
        <button role="menuitem" onclick={() => onElement(() => animateSelection(false))}>Disappear on next click</button>
        <button role="menuitem" onclick={() => onElement(() => { panelOpen = true; panelTab = "anim"; })}>Animations…</button>
      {/if}
      {#if sel.shape === "path"}<button role="menuitem" onclick={() => onElement(() => startEdit(sel!.index))}>Edit points</button>{/if}
    </div>
  {/if}

  {#if renamingItem}
    <div class="modal">
      <div class="dialog confirm rename-dialog">
        <h3>Rename the {renamingItem.kind}</h3>
        <input type="text" bind:value={renamingItem.value}
               onkeydown={(e) => { if (e.key === "Enter") renameItem(); if (e.key === "Escape") renamingItem = null; e.stopPropagation(); }} />
        <p class="muted">{renamingItem.kind === "layout" ? "The slides using it follow." : "Its layouts follow."}</p>
        <div class="row">
          <button class="primary" onclick={renameItem}>Rename</button>
          <button onclick={() => (renamingItem = null)}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  {#if pending}
    <div class="modal">
      <div class="dialog confirm">
        <h3>Unsaved changes</h3>
        <p>The deck has changes that are not saved.</p>
        <div class="row">
          <button class="primary" onclick={() => answer("save")}>Save</button>
          <button onclick={() => answer("discard")}>Don't save</button>
          <button onclick={() => answer("cancel")}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  {#if slideMenu && view}
    {@const m = slideMenu}
    {@const st = view.starts[m.slide] ?? { section: null, subsection: null }}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="menu-backdrop" onclick={() => (slideMenu = null)} oncontextmenu={(e) => { e.preventDefault(); slideMenu = null; }}></div>
    <div class="context-menu" role="menu" style="left: {m.x}px; top: {m.y}px">
      <div class="menu-title">{targets(m.slide).length > 1 ? `${targets(m.slide).length} slides` : `Slide ${m.slide + 1}`}</div>
      <button role="menuitem" aria-expanded={layoutList} onclick={() => (layoutList = !layoutList)}>Layout {layoutList ? "▾" : "▸"}</button>
      {#if layoutList}
        <div class="menu-list">
          {#each view.layout_groups as g}
            {#if g.theme}<div class="menu-group">{g.theme}</div>{/if}
            {#each g.layouts as name}
              <button role="menuitem" onclick={() => applyLayout(name)}>{name}</button>
            {/each}
          {/each}
          <button role="menuitem" onclick={() => applyLayout(null)}><em>No layout</em></button>
        </div>
      {/if}
      {#if targets(m.slide).length === 1}
      <hr />
      <button role="menuitem" onclick={() => onSlide("Added slide", api.addSlide)}>New slide after</button>
      <button role="menuitem" title="A slide listing the sections, the current one highlighted" onclick={() => onSlide("Added plan slide", async () => { await api.addSlide(); await api.addOutline(); })}>New plan slide after</button>
      <button role="menuitem" onclick={() => onSlide("Duplicated slide", api.duplicateSlide)}>Duplicate</button>
      <button role="menuitem" disabled={view.slide_count <= 1} onclick={() => onSlide("Deleted slide", api.deleteSlide)}>Delete</button>
      <hr />
      {#if st.section === null}
        <button role="menuitem" onclick={() => { const k = m.slide; slideMenu = null; startHeading(k, "section"); }}>Start a section here</button>
      {:else}
        <button role="menuitem" onclick={() => removeHeading(m.slide, "section")}>Remove section “{st.section}”</button>
      {/if}
      {#if st.subsection === null}
        <button role="menuitem" onclick={() => { const k = m.slide; slideMenu = null; startHeading(k, "subsection"); }}>Start a subsection here</button>
      {:else}
        <button role="menuitem" onclick={() => removeHeading(m.slide, "subsection")}>Remove subsection “{st.subsection}”</button>
      {/if}
      {/if}
    </div>
  {/if}

  {#if pdfDialog}
    <div class="modal">
      <div class="dialog small">
        <h3>Export PDF</h3>
        <label><input type="radio" name="pdfmode" checked={pdfMode === "slides"} onchange={() => (pdfMode = "slides")} /> One page per slide, as it looks at the end (for reading and printing)</label>
        <label><input type="radio" name="pdfmode" checked={pdfMode === "steps"} onchange={() => (pdfMode = "steps")} /> One page per build step (for presenting with decker-viewer or any PDF reader)</label>
        <label><input type="radio" name="pdfmode" checked={pdfMode === "notes"} onchange={() => (pdfMode = "notes")} /> Handout: one A4 page per slide with its speaker notes below</label>
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
  /* File and the insert tools on the left; undo, redo, help, then the
     presentation on the right. */
  header .spacer { order: 5; }
  header .end { order: 6; }
  header .primary, header :global(.menu-button.primary) { order: 7; }
  .dirty { color: var(--accent); }
  .dialog.confirm { width: 360px; }
  .dialog.confirm h3 { margin: 0 0 6px; }
  .ph-rename { position: absolute; z-index: 6; font-size: 14px; padding: 4px 6px; border: 2px solid var(--accent); border-radius: 4px; background: var(--panel); color: var(--fg); }
  .rename-dialog input { width: 100%; box-sizing: border-box; margin: 6px 0; }
  header { grid-area: head; display: flex; flex-wrap: wrap; gap: 6px; padding: 8px; border-bottom: 1px solid var(--border); align-items: center; }
  header button { white-space: nowrap; }
  .spacer { flex: 1; }
  .banner { color: var(--muted); display: flex; align-items: center; gap: 6px; }
  .layout-name { width: 160px; padding: 3px 6px; }
  .row { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
  .props .row button { white-space: nowrap; font-size: 12px; padding: 3px 8px; width: auto; flex: none; }
  .macros { display: flex; flex-wrap: wrap; gap: 6px; }
  .menu-backdrop { position: fixed; inset: 0; z-index: 40; }
  .context-menu { position: fixed; z-index: 41; min-width: 220px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; padding: 4px; box-shadow: 0 8px 24px rgba(0,0,0,0.45); display: flex; flex-direction: column; }
  .context-menu button { text-align: left; border: none; background: none; padding: 6px 10px; border-radius: 5px; }
  .context-menu button:hover:not(:disabled) { background: var(--accent); color: white; }
  .context-menu button:disabled { opacity: 0.4; cursor: default; }
  .context-menu hr { border: none; border-top: 1px solid var(--border); margin: 4px 2px; }
  .menu-title { font-size: 11px; color: var(--muted); text-transform: uppercase; padding: 4px 10px 2px; }
  /* One row, always the same height: the slide does not jump when the
     selection changes; it scrolls sideways when the window is narrow. */
  .bars { flex: none; height: 40px; display: flex; flex-wrap: nowrap; align-items: center; overflow-x: auto; overflow-y: hidden; border-bottom: 1px solid var(--border); scrollbar-width: thin; }
  .views { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; margin-left: 8px; }
  .views button { border: none; border-radius: 0; padding: 4px 10px; }
  .views button.on { background: var(--accent); color: white; }
  .design-item { display: flex; flex-direction: column; gap: 3px; background: none; padding: 3px; border: 1px solid transparent; border-radius: 6px; text-align: left; }
  .design-item.layout { margin-left: 18px; }
  .design-item.layout.loose { margin-left: 0; }
  .design-item.current { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .design-item :global(.slide-svg) { width: 100%; }
  .design-name { font-size: 11px; color: var(--muted); display: flex; gap: 4px; align-items: center; }
  .design-item.theme .design-name { color: var(--fg); font-weight: 600; }
  .design-item .used { margin-left: auto; background: var(--bg); border-radius: 8px; padding: 0 5px; }
  .pane-hint { font-size: 11px; }
  .fold-tools { flex: none; padding: 0 8px; border-left: 1px solid var(--border); }
  .fold-tools button { font-size: 12px; padding: 2px 8px; white-space: nowrap; }
  .fold-tools button.on { border-color: var(--accent); color: var(--accent); }
  main { position: relative; }
  .arrange-pop { position: absolute; right: 12px; top: 44px; z-index: 41; width: 380px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45); padding: 4px; }
  .arrange-pop :global(.arrangebar) { flex-wrap: wrap; border-left: none; white-space: normal; row-gap: 6px; }
  .bars-hint { padding: 0 16px; color: var(--muted); font-size: 12px; white-space: nowrap; }
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
  .slides { grid-area: slides; overflow-y: auto; padding: 8px; position: relative; border-right: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  .slides.collapsed { padding: 6px 4px; overflow: hidden; align-items: center; }
  .pane-toggle { align-self: flex-end; font-size: 12px; padding: 0 6px; flex: none; }
  .slides.collapsed .pane-toggle { align-self: center; }
  .pane-resizer { grid-area: slides; justify-self: end; width: 6px; cursor: ew-resize; z-index: 3; }
  .pane-resizer:hover { background: var(--accent); opacity: 0.4; }
  .stack-item { position: relative; border-top: 2px solid transparent; border-bottom: 2px solid transparent; cursor: grab; }
  .stack-item.dragging { opacity: 0.4; }
  .stack-item { user-select: none; touch-action: none; }
  .stack-item.drop-before { border-top-color: var(--accent); }
  .stack-item.drop-after { border-bottom-color: var(--accent); }
  .adders button { font-size: 10px; padding: 1px 5px; background: var(--panel); }
  .heading { display: flex; align-items: center; gap: 6px; padding: 6px 2px 0; }
  .heading.section { border-top: 1px solid var(--border); margin-top: 4px; }
  .heading.subsection { padding-left: 12px; }
  .heading-name { background: none; border: none; padding: 0; color: var(--fg); font-weight: 600; font-size: 12px; text-align: left; cursor: text; }
  .heading.subsection .heading-name { font-weight: 500; color: var(--muted); }
  .heading input { font-size: 12px; padding: 2px 4px; }
  /* The number beside the picture, not over its title. */
  .thumb { padding: 2px; position: relative; background: none; width: 100%; display: flex; gap: 4px; align-items: flex-start; }
  .thumb :global(.slide-svg) { flex: 1; min-width: 0; }
  .thumb.picked { outline: 2px dashed var(--accent); outline-offset: 1px; }
  .file-menu { position: relative; }
  .context-menu.file { position: absolute; left: 0; top: calc(100% + 4px); min-width: 240px; }
  .context-menu kbd { float: right; margin-left: 16px; color: var(--muted); font: inherit; font-size: 11px; }
  .menu-group { color: var(--muted); font-size: 11px; text-transform: uppercase; padding: 4px 8px 0; }
  details.kind summary { cursor: pointer; padding: 3px 0; text-transform: capitalize; }
  .menu-list { max-height: 40vh; overflow-y: auto; padding-left: 12px; display: flex; flex-direction: column; }
  .thumb.current { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .thumb .num { flex: none; width: 16px; text-align: right; color: var(--muted); font-size: 11px; padding-top: 1px; }
  .thumb .anim { display: block; color: var(--accent); font-size: 10px; line-height: 1.4; }
  .clicks { display: inline-flex; align-items: center; gap: 4px; }
  .clicks button { font-size: 11px; padding: 1px 7px; }
  .clicks button.on { border-color: var(--accent); color: var(--accent); }
  .clicks .at { min-width: 74px; text-align: center; font-size: 12px; }
  .thumb.current .num { color: var(--accent); font-weight: 600; }
  main { grid-area: main; display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .canvas-wrap { flex: 1; display: flex; align-items: center; justify-content: center; padding: 16px; min-height: 0; }
  .canvas-wrap.zoomed { overflow: auto; align-items: flex-start; justify-content: flex-start; }
  .canvas-wrap.zoomed .canvas { margin: auto; }
  .canvas { position: relative; flex: none; box-shadow: 0 4px 24px rgba(0,0,0,0.5); }
  .canvas.drop-hover { outline: 3px dashed var(--accent); outline-offset: 4px; }
  .overlay { position: absolute; inset: 0; width: 100%; height: 100%; }
  .hit { fill: transparent; stroke: transparent; stroke-width: 1; cursor: move; }
  .hit:hover { stroke: var(--accent); stroke-dasharray: 4 2; }
  .hit.selected { stroke: var(--accent); stroke-width: 2; }
  .hit.thin { stroke: transparent; stroke-width: 14px; vector-effect: non-scaling-stroke; pointer-events: stroke; stroke-linecap: round; }
  .hit.thin:hover, .hit.thin.selected { stroke: var(--accent); stroke-opacity: 0.35; stroke-dasharray: none; }
  .handle { fill: var(--accent); cursor: nwse-resize; }
  .rotate { fill: white; stroke: var(--accent); stroke-width: 2; vector-effect: non-scaling-stroke; cursor: grab; pointer-events: all; }
  .rotate-stem { stroke: var(--accent); stroke-width: 1; vector-effect: non-scaling-stroke; pointer-events: none; }
  .band { fill: rgba(79, 140, 255, 0.12); stroke: var(--accent); stroke-width: 1; vector-effect: non-scaling-stroke; stroke-dasharray: 4 3; pointer-events: none; }
  .text-editing { cursor: text; }
  .edit-frame { fill: none; stroke: var(--accent); stroke-width: 1.5; stroke-dasharray: 4 3; }
  .selection { fill: rgba(79, 140, 255, 0.35); }
  .caret { stroke: black; stroke-width: 1.5; animation: blink 1s steps(2, start) infinite; }
  @keyframes blink { to { visibility: hidden; } }
  .steps { padding: 6px 16px; color: var(--muted); display: flex; gap: 8px; align-items: center; }
  .props { grid-area: props; overflow-y: auto; padding: 12px; border-left: 1px solid var(--border); }
  .props.collapsed { padding: 6px 4px; overflow: hidden; }
  .tabs { display: flex; gap: 4px; margin-bottom: 6px; position: sticky; top: -12px; background: var(--panel, var(--bg)); padding: 4px 0; z-index: 2; }
  .tabs button { font-size: 12px; padding: 3px 8px; }
  .tabs button.on { border-color: var(--accent); color: var(--accent); }
  .tabs .collapse { margin-left: auto; }
  .tabs.vertical { flex-direction: column; align-items: center; position: static; }
  .tabs.vertical button { width: 28px; padding: 4px 0; }
  .props .link { background: none; border: none; color: var(--accent); padding: 0; font-size: inherit; }
  .xywh { display: grid; grid-template-columns: 1fr 1fr; gap: 0 8px; }
  .xywh label { display: flex !important; align-items: center; gap: 6px; }
  .xywh input { min-width: 0; }
  .notes { box-sizing: border-box; width: calc(100% - 32px); margin: 0 16px 10px; resize: vertical; min-height: 40px; font: inherit; }
  .fold { font-size: 12px; padding: 2px 8px; }
  .props h3 { margin: 12px 0 6px; font-size: 13px; color: var(--muted); text-transform: uppercase; }
  .props label { display: block; margin-bottom: 6px; color: var(--muted); }
  .props label.check { display: flex; align-items: center; gap: 6px; }
  .props label.check input { width: auto; }
  .muted { color: var(--muted); }
  .crop-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 8px; }
  .empty { grid-area: main; display: flex; flex-direction: column; gap: 18px; align-items: center; justify-content: center; color: var(--muted); grid-column: 1 / -1; }
  .empty .logo { width: min(640px, 70vw); height: auto; }
  .empty .row { justify-content: center; }
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
  footer { grid-area: foot; padding: 2px 12px; border-top: 1px solid var(--border); color: var(--muted); font-size: 12px; min-height: 24px; display: flex; align-items: center; gap: 8px; }
  footer .status { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  footer .zoom { display: inline-flex; gap: 2px; align-items: center; }
  footer .zoom button { font-size: 11px; padding: 0 6px; height: 20px; }
  footer .zoom button.on { border-color: var(--accent); color: var(--accent); }
  footer .pct { width: 40px; text-align: right; }
</style>
