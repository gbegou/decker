<script lang="ts">
  // One page of the PDF, drawn into a canvas. The box is sized here, in
  // pixels, from its parent: to the parent's width (`fit="width"`,
  // thumbnails and strips) or into the parent's width and height
  // (`fit="box"`, the presenter's stage), so that nothing overflows.
  import { render } from "./pdf";
  let { path, index, width, height, fit = "width" }: {
    path: string; index: number; width: number; height: number; fit?: "width" | "box";
  } = $props();
  let box = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let size = $state({ w: 0, h: 0 });

  function measure() {
    const parent = box?.parentElement;
    if (!parent) return;
    const style = getComputedStyle(parent);
    const pw = parent.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    const ph = parent.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom);
    let w = Math.max(pw, 0), h = (w * height) / width;
    if (fit === "box" && ph > 0 && h > ph) { h = ph; w = (h * width) / height; }
    if (Math.abs(w - size.w) > 0.5 || Math.abs(h - size.h) > 0.5) size = { w, h };
  }
  $effect(() => {
    if (!box?.parentElement) return;
    const ro = new ResizeObserver(measure);
    ro.observe(box.parentElement);
    measure();
    return () => ro.disconnect();
  });
  $effect(() => {
    const p = path, i = index, w = size.w, c = canvas;
    if (c && w > 0) void render(p, i, c, w);
  });
</script>

<div class="page" bind:this={box} style="width: {size.w}px; height: {size.h}px">
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .page { background: white; overflow: hidden; flex: none; }
  canvas { display: block; width: 100%; height: 100%; }
</style>
