// pdf.js in the web view: one document per path, pages rendered to
// canvases. The worker is bundled by Vite; the file is read through
// Tauri's asset protocol.
import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { convertFileSrc } from "@tauri-apps/api/core";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

const docs = new Map<string, Promise<pdfjs.PDFDocumentProxy>>();

/** The document at `path`, loaded once per window. */
export function document(path: string): Promise<pdfjs.PDFDocumentProxy> {
  let d = docs.get(path);
  if (!d) {
    d = pdfjs.getDocument({ url: convertFileSrc(path) }).promise;
    docs.set(path, d);
  }
  return d;
}

/** Page count and the first page's size in points. */
export async function info(path: string): Promise<{ pages: number; width: number; height: number }> {
  const doc = await document(path);
  const first = await doc.getPage(1);
  const [, , w, h] = first.view;
  return { pages: doc.numPages, width: w, height: h };
}

// One render at a time per canvas, in order, and only the latest
// requested page: a draw started while another runs on the same canvas
// would fail or leave the context transformed.
const chains = new WeakMap<HTMLCanvasElement, Promise<void>>();
const latest = new WeakMap<HTMLCanvasElement, symbol>();

/** Draws page `index` (0-based) into `canvas` at the canvas's CSS width, crisp on HiDPI. */
export function render(path: string, index: number, canvas: HTMLCanvasElement, cssWidth: number): Promise<void> {
  const token = Symbol();
  latest.set(canvas, token);
  const next = (chains.get(canvas) ?? Promise.resolve())
    .then(() => (latest.get(canvas) === token ? draw(path, index, canvas, cssWidth) : undefined))
    .catch(() => {});
  chains.set(canvas, next);
  return next;
}

async function draw(path: string, index: number, canvas: HTMLCanvasElement, cssWidth: number): Promise<void> {
  const doc = await document(path);
  if (index < 0 || index >= doc.numPages) return;
  const page = await doc.getPage(index + 1);
  const base = page.getViewport({ scale: 1 });
  const ratio = window.devicePixelRatio || 1;
  const scale = (cssWidth * ratio) / base.width;
  const viewport = page.getViewport({ scale });
  canvas.width = Math.round(viewport.width);
  canvas.height = Math.round(viewport.height);
  // The canvas follows its box; only the bitmap is sized here.
  canvas.style.width = "100%";
  canvas.style.height = "auto";
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  await page.render({ canvasContext: ctx, viewport, canvas }).promise;
}
