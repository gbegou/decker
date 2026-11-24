<script lang="ts">
  // Shows a slide's SVG, letterboxed in its box. When `animated`, a new
  // step patches the existing <g> elements instead of replacing the
  // document, so CSS transitions run on opacity and rotation, and a
  // playing video is left alone. Videos and GIFs are pointed at the
  // deck folder `dir` and, in `show` mode, played.
  import { wireMedia, type MediaMode } from "./media";
  let { svg, width, height, animated = false, dir = "", mode = "editor" }: {
    svg: string; width: number; height: number; animated?: boolean; dir?: string; mode?: MediaMode;
  } = $props();
  let host: HTMLDivElement;

  // The markup each <g> was last given, to tell real changes from the
  // src rewriting done on the live DOM.
  const given = new WeakMap<Element, string>();

  $effect(() => {
    if (!host) return;
    const current = host.querySelector("svg");
    const replace = () => {
      host.innerHTML = svg;
      for (const g of host.querySelectorAll(":scope > svg > g")) given.set(g, g.innerHTML);
      if (dir) wireMedia(host, dir, mode);
    };
    if (!animated || !current) {
      replace();
      return;
    }
    const next = new DOMParser().parseFromString(svg, "image/svg+xml").documentElement;
    const nextGroups = Array.from(next.querySelectorAll(":scope > g"));
    const same = nextGroups.length === current.querySelectorAll(":scope > g").length;
    if (!same) {
      replace();
      return;
    }
    for (const g of nextGroups) {
      const old = current.querySelector(`:scope > g#${g.id}`);
      if (!old) continue;
      if (given.get(old) !== g.innerHTML) { old.innerHTML = g.innerHTML; given.set(old, g.innerHTML); }
      for (const attr of ["opacity", "transform"]) {
        const v = g.getAttribute(attr);
        if (v === null) old.removeAttribute(attr); else old.setAttribute(attr, v);
      }
    }
    if (dir) wireMedia(host, dir, mode);
  });
</script>

<div class="slide-svg" class:animated bind:this={host} style="aspect-ratio: {width} / {height}"></div>

<style>
  .slide-svg { width: 100%; max-height: 100%; background: white; }
</style>
