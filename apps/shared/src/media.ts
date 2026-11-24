// Videos and animated GIFs on a rendered slide. The renderer emits them
// as `<foreignObject>` holding a `<video>` or an `<img>` with deck-relative
// paths in `data-` attributes; this module points them at the deck's
// files, starts them when they appear or when their play button is
// clicked, and stops single-play GIFs on their poster after their length.
import { convertFileSrc } from "@tauri-apps/api/core";

export type MediaMode = "editor" | "show";

interface Player {
  started: boolean;
  timer?: ReturnType<typeof setTimeout>;
}

const players = new WeakMap<Element, Player>();

/** The URL of a deck-relative asset path. */
export function assetUrl(dir: string, rel: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  return convertFileSrc(dir + sep + rel.replace(/\//g, sep));
}

/**
 * Wires the media of `host` (the slide's root). Called after each render
 * or step patch; elements already wired keep their playback state.
 * In `show` mode, media that just became visible start when set to play
 * on appearance; in `editor` mode nothing plays.
 */
export function wireMedia(host: HTMLElement, dir: string, mode: MediaMode) {
  for (const el of Array.from(host.querySelectorAll<HTMLElement>("[data-media]"))) {
    const rel = el.dataset.media!;
    const poster = el.dataset.poster || "";
    const isGif = el.tagName.toLowerCase() === "img";
    let player = players.get(el);
    if (!player) {
      player = { started: false };
      players.set(el, player);
      if (isGif) {
        // The poster until it plays; without a poster, the GIF itself (looping as the file says).
        el.setAttribute("src", poster ? assetUrl(dir, poster) : assetUrl(dir, rel));
      } else {
        el.setAttribute("src", assetUrl(dir, rel));
        if (poster) el.setAttribute("poster", assetUrl(dir, poster));
        (el as HTMLVideoElement).muted = el.dataset.sound !== "1";
      }
      const button = el.parentElement?.nextElementSibling;
      if (button?.classList.contains("media-play")) {
        button.addEventListener("click", (e) => {
          e.stopPropagation();
          if (mode === "show") start(el, dir, true);
        });
      }
    }
    if (mode !== "show") continue;
    const visible = isShown(el);
    if (visible && !player.started && el.dataset.play === "appear") start(el, dir, false);
    if (!visible && player.started) reset(el, dir);
  }
}

/** Starts (or restarts) one media element. */
function start(el: HTMLElement, dir: string, restart: boolean) {
  const player = players.get(el);
  if (!player) return;
  if (player.started && !restart) return;
  player.started = true;
  if (player.timer) clearTimeout(player.timer);
  const button = el.parentElement?.nextElementSibling as HTMLElement | null;
  if (button?.classList.contains("media-play")) button.style.display = "none";
  const loop = el.dataset.loop === "1";
  const duration = parseFloat(el.dataset.duration || "") || 0;
  if (el.tagName.toLowerCase() === "img") {
    // A fresh URL restarts the GIF from its first frame.
    el.setAttribute("src", assetUrl(dir, el.dataset.media!) + "?t=" + Date.now());
    if (!loop && duration > 0 && el.dataset.poster) {
      player.timer = setTimeout(() => {
        el.setAttribute("src", assetUrl(dir, el.dataset.poster!));
      }, duration * 1000);
    }
  } else {
    const video = el as HTMLVideoElement;
    video.loop = loop;
    video.currentTime = 0;
    void video.play().catch(() => {});
  }
}

/** Back to the initial state when the element is hidden again (a disappear step, a slide change). */
function reset(el: HTMLElement, dir: string) {
  const player = players.get(el);
  if (!player) return;
  player.started = false;
  if (player.timer) clearTimeout(player.timer);
  const button = el.parentElement?.nextElementSibling as HTMLElement | null;
  if (button?.classList.contains("media-play")) button.style.display = "";
  if (el.tagName.toLowerCase() === "img") {
    if (el.dataset.poster) el.setAttribute("src", assetUrl(dir, el.dataset.poster));
  } else {
    const video = el as HTMLVideoElement;
    video.pause();
    video.currentTime = 0;
  }
}

/** Whether the element's `<g>` is drawn (opacity above zero on every ancestor group). */
function isShown(el: Element): boolean {
  let node: Element | null = el.parentElement;
  while (node && node.tagName.toLowerCase() !== "svg") {
    const o = node.getAttribute("opacity");
    if (o !== null && parseFloat(o) <= 0.01) return false;
    node = node.parentElement;
  }
  return true;
}
