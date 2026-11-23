<script lang="ts">
  // Shows a slide's SVG, letterboxed in its box. When `animated`, a new
  // step patches the existing <g> elements instead of replacing the
  // document, so CSS transitions run on opacity and rotation.
  let { svg, width, height, animated = false }: { svg: string; width: number; height: number; animated?: boolean } = $props();
  let host: HTMLDivElement;

  $effect(() => {
    if (!host) return;
    const current = host.querySelector("svg");
    if (!animated || !current) {
      host.innerHTML = svg;
      return;
    }
    const next = new DOMParser().parseFromString(svg, "image/svg+xml").documentElement;
    const nextGroups = Array.from(next.querySelectorAll(":scope > g"));
    const same = nextGroups.length === current.querySelectorAll(":scope > g").length;
    if (!same) {
      host.innerHTML = svg;
      return;
    }
    for (const g of nextGroups) {
      const old = current.querySelector(`:scope > g#${g.id}`);
      if (!old) continue;
      if (old.innerHTML !== g.innerHTML) old.innerHTML = g.innerHTML;
      for (const attr of ["opacity", "transform"]) {
        const v = g.getAttribute(attr);
        if (v === null) old.removeAttribute(attr); else old.setAttribute(attr, v);
      }
    }
  });
</script>

<div class="slide-svg" class:animated bind:this={host} style="aspect-ratio: {width} / {height}"></div>

<style>
  .slide-svg { width: 100%; max-height: 100%; background: white; }
</style>
