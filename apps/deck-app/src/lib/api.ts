// The calls to the Rust side. One command per user action; every command
// returns the new View, and Rust also emits "view-changed" to every window.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Rect { x: number; y: number; w: number; h: number }
export interface Crop { left: number; top: number; right: number; bottom: number }
export type Mask = { kind: "rectangle" } | { kind: "ellipse" } | { kind: "rounded_rectangle"; radius: number };

export interface ElementInfo {
  index: number;
  kind: "text" | "image" | "shape" | "math";
  rect: Rect;
  text: string;
  last_step: number;
  size: number;
  crop?: Crop;
  mask?: Mask;
}

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
  exportPdf: (path: string) => invoke<void>("export_pdf", { path }),
  setSection: (section: string | null, subsection: string | null) => invoke<View>("set_section", { section, subsection }),
  setSectionOf: (slide: number, section: string | null, subsection: string | null) =>
    invoke<View>("set_section_of", { slide, section, subsection }),
  getView: () => invoke<View>("get_view"),
  getThumbnails: () => invoke<string[]>("get_thumbnails"),
  navigate: (action: "next" | "prev" | "first" | "last" | "next_slide" | "prev_slide" | "black" | "white") =>
    invoke<View>("navigate", { action }),
  goto: (slide: number, step = 0) => invoke<View>("goto", { slide, step }),
  setRect: (element: number, rect: Rect) => invoke<View>("set_rect", { element, rect }),
  setText: (element: number, text: string) => invoke<View>("set_text", { element, text }),
  setNotes: (notes: string) => invoke<View>("set_notes", { notes }),
  setImage: (element: number, crop: Crop, mask: Mask) => invoke<View>("set_image", { element, crop, mask }),
  textLayout: (element: number) => invoke<TextLayout | null>("text_layout", { element }),
  textEdit: (element: number, from: number, to: number, insert: string) => invoke<number>("text_edit", { element, from, to, insert }),
  addSlide: () => invoke<View>("add_slide"),
  deleteSlide: () => invoke<View>("delete_slide"),
  duplicateSlide: () => invoke<View>("duplicate_slide"),
  setLayout: (name: string | null) => invoke<View>("set_layout", { name }),
  peekDeck: (path: string) => invoke<string[]>("peek_deck", { path }),
  importSlides: (path: string, indices: number[]) => invoke<View>("import_slides", { path, indices }),
  addText: () => invoke<View>("add_text"),
  deleteElement: (element: number) => invoke<View>("delete_element", { element }),
  startPresentation: (withPresenter = false) => invoke<void>("start_presentation", { withPresenter }),
  stopPresentation: () => invoke<void>("stop_presentation"),
};

/** Calls `cb` with the view now and after every change, until unsubscribed. */
export function watchView(cb: (v: View) => void): () => void {
  let alive = true;
  api.getView().then((v) => alive && cb(v));
  const un = listen("view-changed", async () => {
    const v = await api.getView();
    if (alive) cb(v);
  });
  return () => {
    alive = false;
    un.then((f) => f());
  };
}

/** Calls `cb` with the thumbnails now and after every content change. */
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

/** Keyboard handling shared by the presentation windows and the editor. */
export function presentationKeys(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "ArrowRight": case "ArrowDown": case "PageDown": case " ": case "Enter":
      api.navigate("next"); return true;
    case "ArrowLeft": case "ArrowUp": case "PageUp": case "Backspace":
      api.navigate("prev"); return true;
    case "Home": api.navigate("first"); return true;
    case "End": api.navigate("last"); return true;
    case "b": case "B": case ".": api.navigate("black"); return true;
    case "w": case "W": case ",": api.navigate("white"); return true;
    case "Escape": api.stopPresentation(); return true;
  }
  return false;
}
