// The calls to the Rust side. One command per user action; every command
// returns the new View, and Rust also emits "view-changed" to every window.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { watchView as watchShowView, presentationKeys } from "$shared";

export interface Rect { x: number; y: number; w: number; h: number }
export interface Crop { left: number; top: number; right: number; bottom: number }
export type Mask = { kind: "rectangle" } | { kind: "ellipse" } | { kind: "rounded_rectangle"; radius: number };

export interface ElementInfo {
  index: number;
  kind: "text" | "image" | "media" | "figure" | "table" | "shape" | "math" | "placeholder" | "outline" | "group";
  rect: Rect;
  /** Degrees clockwise about the box's center, at the viewed click. */
  rotation: number;
  text: string;
  last_step: number;
  size: number;
  crop?: Crop;
  mask?: Mask;
  steps: Step[];
  fill?: string;
  stroke?: { color: string; width: number };
  highlight?: string;
  subsections?: boolean;
  media?: MediaInfo;
  /** Shapes: their kind, and whether a line or arrow is flipped. */
  shape?: "rectangle" | "ellipse" | "line" | "arrow";
  flip: boolean;
  /** Placeholders: their default text style. */
  placeholder?: PlaceholderStyle & { name: string };
  /** Tables: the cells' texts and fills by row, and the settings. */
  table?: { cells: string[][]; fills: (string | null)[][]; header: boolean; columns: number[] };
}
/** An edit of a table: one cell, the grid, the header, the column widths. */
export type TableEdit =
  | { op: "cell"; row: number; column: number; text: string }
  | { op: "fill"; row: number; column: number; color: string }
  | { op: "add_row" | "add_column" | "remove_row" | "remove_column"; at: number }
  | { op: "header"; on: boolean }
  | { op: "columns"; widths: number[] };
export interface PlaceholderStyle {
  size?: number; bold?: boolean; color?: string; font?: string; align?: Align; bullets?: boolean;
}

export type Align = "left" | "center" | "right" | "justify";

/** A build step: what changes at click `at`, played over `duration` seconds after `delay`. */
export interface Step {
  at: number;
  set: { x?: number; y?: number; w?: number; h?: number; rotation?: number; opacity?: number; fill?: string };
  duration?: number;
  delay?: number;
  easing?: "ease_in_out" | "linear" | "ease_in" | "ease_out";
}

/** A run style change; absent fields keep their value; size 0, color "" and font "" reset. */
export interface StyleChange {
  bold?: boolean; italic?: boolean; toggle_bold?: boolean; toggle_italic?: boolean;
  size?: number; color?: string; font?: string;
}
export interface ParagraphChange { align?: Align; bullet?: number }
export interface StyleInfo {
  bold: boolean; italic: boolean; size: number | null; color: string | null; font: string | null; align: Align; bullet: number | null;
}
export interface ElementStyle {
  fill?: string; stroke?: string; stroke_width?: number; size?: number; highlight?: string; subsections?: boolean;
  /** Media: "appear" or "button"; loop; sound. */
  play?: "appear" | "button"; looping?: boolean; sound?: boolean;
  /** Lines and arrows: run bottom-left to top-right. */
  flip?: boolean;
}
export interface MediaInfo { play: "on_appear" | "button"; looping: boolean; sound: boolean; duration: number | null; poster: string | null }

export interface Theme {
  font?: string; size?: number; color?: string; accent?: string; palette?: string[];
}
export interface ThemeChange {
  font?: string; size?: number; color?: string; accent?: string; palette?: string[];
}
/** `own`: what the edited deck or layout sets; `effective`: with the deck's fallback. */
export interface ThemeInfo { own: Theme; effective: Theme; fonts: string[] }

export interface View {
  title: string;
  dir: string;
  width: number;
  height: number;
  slide: number;
  slide_count: number;
  step: number;
  last_step: number;
  svg: string;
  next_svg: string | null;
  notes: string;
  elements: ElementInfo[];
  presenting: boolean;
  /** "", "black" or "white": the audience window shows that instead of the slide. */
  blank: string;
  warnings: string[];
  layouts: string[];
  layout: string | null;
  section: string | null;
  subsection: string | null;
  in_section: string | null;
  in_subsection: string | null;
  starts: { section: string | null; subsection: string | null }[];
  /** The layout shown on the canvas instead of a slide, when editing one. */
  editing_layout: string | null;
  theme: ThemeInfo;
  can_undo: boolean;
  can_redo: boolean;
}

/** Character boxes of a text element, in slide points; the last is the caret after the end. */
export interface TextLayout {
  text: string;
  chars: { x: number; right: number; top: number; bottom: number }[];
}

export const api = {
  open: (path: string) => invoke<View>("open", { path }),
  newDeck: () => invoke<View>("new_deck"),
  save: (path?: string) => invoke<string>("save", { path: path ?? null }),
  exportPptx: (path: string) => invoke<string[]>("export_pptx", { path }),
  /** `slides`: a page per slide; `steps`: a page per build step; `notes`: the handout with notes. */
  exportPdf: (path: string, mode: "slides" | "steps" | "notes" = "slides") => invoke<void>("export_pdf", { path, mode }),
  setSection: (section: string | null, subsection: string | null) => invoke<View>("set_section", { section, subsection }),
  setSectionOf: (slide: number, section: string | null, subsection: string | null) =>
    invoke<View>("set_section_of", { slide, section, subsection }),
  getView: () => invoke<View>("get_view"),
  undo: () => invoke<View>("undo"),
  /** The deck's macros (scripts/*.py) and the Python found, if any. */
  listScripts: () => invoke<{ scripts: string[]; python: string | null }>("list_scripts"),
  /** Runs a macro (a name in scripts/ or an absolute path) against the open deck. */
  runScript: (name: string, selection: number[]) =>
    invoke<{ ok: boolean; changed: boolean; stdout: string; stderr: string; python: string }>("run_script", { name, selection }),
  redo: () => invoke<View>("redo"),
  getThumbnails: () => invoke<string[]>("get_thumbnails"),
  navigate: (action: "next" | "prev" | "first" | "last" | "next_slide" | "prev_slide" | "black" | "white") =>
    invoke<View>("navigate", { action }),
  goto: (slide: number, step = 0) => invoke<View>("goto", { slide, step }),
  setRect: (element: number, rect: Rect) => invoke<View>("set_rect", { element, rect }),
  /** Turns an element to `degrees` clockwise at the viewed click. */
  setRotation: (element: number, degrees: number) => invoke<View>("set_rotation", { element, degrees }),
  /** Moves several elements as one undo step. */
  setRects: (rects: [number, Rect][]) => invoke<View>("set_rects", { rects }),
  /** Aligns, distributes, mirrors or reorders elements; returns their indices afterwards. */
  arrange: (indices: number[], op: string) => invoke<number[]>("arrange", { indices, op }),
  setText: (element: number, text: string) => invoke<View>("set_text", { element, text }),
  setNotes: (notes: string) => invoke<View>("set_notes", { notes }),
  setImage: (element: number, crop: Crop, mask: Mask) => invoke<View>("set_image", { element, crop, mask }),
  textLayout: (element: number) => invoke<TextLayout | null>("text_layout", { element }),
  textEdit: (element: number, from: number, to: number, insert: string) => invoke<number>("text_edit", { element, from, to, insert }),
  addSlide: () => invoke<View>("add_slide"),
  deleteSlide: () => invoke<View>("delete_slide"),
  duplicateSlide: () => invoke<View>("duplicate_slide"),
  moveSlide: (from: number, to: number) => invoke<View>("move_slide", { from, to }),
  editLayout: (name: string | null) => invoke<View>("edit_layout", { name }),
  newLayout: () => invoke<View>("new_layout"),
  renameLayout: (name: string) => invoke<View>("rename_layout", { name }),
  deleteLayout: () => invoke<View>("delete_layout"),
  addPlaceholder: () => invoke<View>("add_placeholder"),
  addOutline: () => invoke<View>("add_outline"),
  addFigure: () => invoke<View>("add_figure"),
  /** A video's poster from a captured frame (PNG as base64), or null to remove it. */
  setPoster: (element: number, data: string | null) => invoke<View>("set_poster", { element, data }),
  /** Adds a table and returns its index. */
  addTable: (rows: number, columns: number) => invoke<number>("add_table", { rows, columns }),
  editTable: (element: number, edit: TableEdit) => invoke<View>("edit_table", { element, edit }),
  /** Adds a shape and returns its index. */
  addShape: (kind: "rectangle" | "ellipse" | "line" | "arrow") => invoke<number>("add_shape", { kind }),
  stylePlaceholder: (element: number, change: PlaceholderStyle) => invoke<View>("style_placeholder", { element, change }),
  groupElements: (indices: number[]) => invoke<number>("group_elements", { indices }),
  setSteps: (element: number, steps: Step[]) => invoke<View>("set_steps", { element, steps }),
  setTheme: (change: ThemeChange) => invoke<View>("set_theme", { change }),
  ungroupElement: (index: number) => invoke<number>("ungroup_element", { index }),
  styleText: (element: number, from: number | null, to: number | null, change: StyleChange) =>
    invoke<View>("style_text", { element, from, to, change }),
  styleParagraphs: (element: number, from: number | null, to: number | null, change: ParagraphChange) =>
    invoke<View>("style_paragraphs", { element, from, to, change }),
  styleInfo: (element: number, from: number | null, to: number | null) => invoke<StyleInfo>("style_info", { element, from, to }),
  styleElement: (element: number, change: ElementStyle) => invoke<View>("style_element", { element, change }),
  saveLayout: (path: string) => invoke<string>("save_layout", { path }),
  loadLayout: (path: string) => invoke<View>("load_layout", { path }),
  setLayout: (name: string | null) => invoke<View>("set_layout", { name }),
  peekDeck: (path: string) => invoke<string[]>("peek_deck", { path }),
  importSlides: (path: string, indices: number[]) => invoke<View>("import_slides", { path, indices }),
  addText: () => invoke<View>("add_text"),
  /** Adds an image from bytes (base64), centered at (x, y) in slide points when given; returns its index. */
  addImage: (name: string, data: string, x?: number, y?: number) =>
    invoke<number>("add_image", { name, data, x: x ?? null, y: y ?? null }),
  /** Copies elements of the current slide to the system clipboard. */
  copyElements: (indices: number[]) => invoke<void>("copy_elements", { indices }),
  /** Pastes the system clipboard: copied elements or an image (their indices), else its text. */
  paste: () => invoke<{ added: number[]; text: string | null }>("paste"),
  /** Adds an image from a file on disk (a drop); returns its index. */
  addImageFile: (path: string, x?: number, y?: number) => invoke<number>("add_image_file", { path, x: x ?? null, y: y ?? null }),
  deleteElement: (element: number) => invoke<View>("delete_element", { element }),
  startPresentation: (withPresenter = false) => invoke<void>("start_presentation", { withPresenter }),
  stopPresentation: () => invoke<void>("stop_presentation"),
};

/** Calls `cb` with the view now and after every change, until unsubscribed. */
/** The view now and after every change (see the shared `watchView`), typed as the editor's View. */
export function watchView(cb: (v: View) => void): () => void {
  return watchShowView<View>(cb);
}

export function watchThumbnails(cb: (t: string[]) => void): () => void {
  let alive = true;
  api.getThumbnails().then((t) => alive && cb(t));
  const un = listen("deck-changed", async () => {
    const t = await api.getThumbnails();
    if (alive) cb(t);
  });
  return () => {
    alive = false;
    un.then((f) => f());
  };
}

export { presentationKeys };
