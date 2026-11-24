// The presentation protocol shared by the editor and the viewer: the
// commands both apps implement under the same names, the part of the
// view the presentation windows read, and the keys that drive a show.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** What the audience and presenter windows need from the application's view. */
export interface ShowView {
  /** The deck folder (or the PDF's folder), for asset paths. */
  dir: string;
  width: number;
  height: number;
  /** The page shown (a slide, a PDF page) and how many there are. */
  slide: number;
  slide_count: number;
  step: number;
  last_step: number;
  /** The page as SVG when the application renders pages itself, else null. */
  svg: string | null;
  next_svg: string | null;
  notes: string;
  presenting: boolean;
  /** "", "black" or "white": the audience shows that instead of the page. */
  blank: string;
  in_section?: string | null;
  in_subsection?: string | null;
}

export type Navigation = "next" | "prev" | "first" | "last" | "next_slide" | "prev_slide" | "black" | "white";

export const show = {
  getView: <V extends ShowView = ShowView>() => invoke<V>("get_view"),
  navigate: (action: Navigation) => invoke<ShowView>("navigate", { action }),
  goto: (slide: number, step = 0) => invoke<ShowView>("goto", { slide, step }),
  startPresentation: (withPresenter = false) => invoke<void>("start_presentation", { withPresenter }),
  stopPresentation: () => invoke<void>("stop_presentation"),
  monitors: () => invoke<string[]>("monitors"),
};

/**
 * Calls `cb` with the view now and after every `view-changed` event.
 * Responses are sequence-numbered so that a slow one never overwrites a
 * newer one.
 */
export function watchView<V extends ShowView = ShowView>(cb: (v: V) => void): () => void {
  let alive = true;
  let seq = 0, shown = 0;
  const fetch = async () => {
    const n = ++seq;
    const v = await invoke<V>("get_view");
    if (alive && n > shown) { shown = n; cb(v); }
  };
  void fetch();
  const un = listen("view-changed", fetch);
  return () => {
    alive = false;
    un.then((f) => f());
  };
}

/** Keyboard handling shared by the presentation windows and the editor. */
export function presentationKeys(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "ArrowRight": case "ArrowDown": case "PageDown": case " ": case "Enter":
      show.navigate("next"); return true;
    case "ArrowLeft": case "ArrowUp": case "PageUp": case "Backspace":
      show.navigate("prev"); return true;
    case "Home": show.navigate("first"); return true;
    case "End": show.navigate("last"); return true;
    case "b": case "B": case ".": show.navigate("black"); return true;
    case "w": case "W": case ",": show.navigate("white"); return true;
    case "Escape": show.stopPresentation(); return true;
  }
  return false;
}
