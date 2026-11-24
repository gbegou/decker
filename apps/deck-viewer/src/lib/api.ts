import { invoke } from "@tauri-apps/api/core";
import { watchView as watchShowView, type ShowView } from "$shared";

/** The viewer's view: the show protocol plus the PDF path. */
export interface View extends ShowView {
  pdf: string | null;
}

export const api = {
  open: (path: string) => invoke<View>("open", { path }),
  setPages: (pages: number, width: number, height: number) => invoke<View>("set_pages", { pages, width, height }),
  setNotes: (page: number, text: string) => invoke<View>("set_notes", { page, text }),
};

export function watchView(cb: (v: View) => void): () => void {
  return watchShowView<View>(cb);
}
