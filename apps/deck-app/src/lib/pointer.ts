// The presenter's pointer: the mouse over the current-slide miniature,
// mirrored as a dot on the audience window. Sent window to window as an
// event in slide fractions (0..1), not through the deck state, so it
// costs nothing but an IPC message per mouse move.
import { emit, listen } from "@tauri-apps/api/event";

export interface Pointer { x: number; y: number }

export function sendPointer(p: Pointer | null) {
  void emit("pointer", p);
}

/** Calls `cb` with the pointer's position, or null when it is hidden. */
export function watchPointer(cb: (p: Pointer | null) => void): () => void {
  const un = listen<Pointer | null>("pointer", (e) => cb(e.payload));
  return () => { un.then((f) => f()); };
}

/** The pointer's fractions for a mouse event over `box` (the slide's element); null outside. */
export function pointerIn(e: PointerEvent, box: Element | null): Pointer | null {
  if (!box) return null;
  const r = box.getBoundingClientRect();
  const x = (e.clientX - r.left) / r.width, y = (e.clientY - r.top) / r.height;
  return x >= 0 && x <= 1 && y >= 0 && y <= 1 ? { x, y } : null;
}
