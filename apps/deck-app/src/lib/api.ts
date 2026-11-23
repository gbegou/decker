// The calls to the Rust side. One command per user action; every command
// returns the new View, and Rust also emits "view-changed" to every window.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Rect { x: number; y: number; w: number; h: number }

export interface ElementInfo {
  index: number;
  kind: "text" | "image" | "shape" | "math";
  rect: Rect;
  text: string;
  last_step: number;
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
  thumbnails: string[];
  notes: string;
  elements: ElementInfo[];
  presenting: boolean;
  blackout: boolean;
  warnings: string[];
}

export const api = {
  open: (path: string) => invoke<View>("open", { path }),
  newDeck: () => invoke<View>("new_deck"),
  save: (path?: string) => invoke<string>("save", { path: path ?? null }),
  exportPptx: (path: string) => invoke<string[]>("export_pptx", { path }),
  getView: () => invoke<View>("get_view"),
  navigate: (action: "next" | "prev" | "first" | "last" | "next_slide" | "prev_slide" | "blackout") =>
    invoke<View>("navigate", { action }),
  goto: (slide: number, step = 0) => invoke<View>("goto", { slide, step }),
  setRect: (element: number, rect: Rect) => invoke<View>("set_rect", { element, rect }),
  setText: (element: number, text: string) => invoke<View>("set_text", { element, text }),
  setNotes: (notes: string) => invoke<View>("set_notes", { notes }),
  addSlide: () => invoke<View>("add_slide"),
  deleteSlide: () => invoke<View>("delete_slide"),
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

/** Keyboard handling shared by the presentation windows and the editor. */
export function presentationKeys(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "ArrowRight": case "ArrowDown": case "PageDown": case " ": case "Enter":
      api.navigate("next"); return true;
    case "ArrowLeft": case "ArrowUp": case "PageUp": case "Backspace":
      api.navigate("prev"); return true;
    case "Home": api.navigate("first"); return true;
    case "End": api.navigate("last"); return true;
    case "b": case "B": case ".": api.navigate("blackout"); return true;
    case "Escape": api.stopPresentation(); return true;
  }
  return false;
}
