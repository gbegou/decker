<script lang="ts">
  // Drawing and editing paths on the canvas, over the slide.
  //   free:   drag to draw; the stroke is simplified and smoothed, and
  //           closes when it ends near its start.
  //   bezier: click a corner, click and drag a smooth node; click the first
  //           node to close, Enter, Escape or a double-click to end open.
  //   edit:   drag nodes and handles (a smooth node keeps its handles in
  //           line, Alt breaks them), double-click a node to switch corner /
  //           smooth, Delete removes the selected node, Escape or a click
  //           off the path ends.
  import { fromStroke, pathData, rotate, toggleSmooth, type PathNode } from "./paths";

  type Pt = { x: number; y: number };
  let {
    mode,
    width,
    height,
    unit,
    toSlide,
    edit = null,
    oncommit,
    ondone,
  }: {
    mode: "free" | "bezier" | "edit";
    width: number;
    height: number;
    /** Slide points per screen pixel: handles stay the same size on screen. */
    unit: number;
    toSlide: (e: PointerEvent | MouseEvent) => Pt;
    /** Edit mode: the path's nodes (slide points), whether it is closed, its rotation and box. */
    edit?: { nodes: PathNode[]; closed: boolean; rotation: number; rect: { x: number; y: number; w: number; h: number } } | null;
    /** A path is drawn, or edited: its nodes in slide points. */
    oncommit: (nodes: PathNode[], closed: boolean) => void;
    ondone: () => void;
  } = $props();

  // ----- drawing -----
  let stroke = $state<Pt[]>([]);
  let nodes = $state<PathNode[]>([]);
  let cursor = $state<Pt | null>(null);
  /** Bézier mode: the node being placed, while the button is down. */
  let placing = $state<number | null>(null);
  /** The button is down (tracked here: some engines report no buttons on moves). */
  let pressed = false;

  // ----- editing -----
  let local = $state<PathNode[]>([]);
  let closed = $state(false);
  let picked = $state<number | null>(null);
  let drag: { node: number; part: "node" | "in" | "out"; from: Pt; start: PathNode } | null = null;

  // Edit mode follows the path as the backend returns it, except mid-drag.
  $effect(() => {
    if (mode === "edit" && edit && !drag) {
      local = edit.nodes.map((n) => ({ ...n }));
      closed = edit.closed;
    }
  });

  const center = $derived(edit ? { x: edit.rect.x + edit.rect.w / 2, y: edit.rect.y + edit.rect.h / 2 } : { x: 0, y: 0 });
  /** A pointer in the path's own (unrotated) space. */
  const at = (e: PointerEvent | MouseEvent): Pt => (mode === "edit" && edit ? rotate(toSlide(e), center, -edit.rotation) : toSlide(e));
  const near = (a: Pt, b: Pt, px = 8) => Math.hypot(a.x - b.x, a.y - b.y) <= px * unit;

  function finishBezier(close: boolean) {
    const done = nodes;
    placing = null;
    if (done.length >= 2) oncommit(done, close && done.length > 2);
    nodes = [];
    ondone();
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    pressed = true;
    const p = at(e);
    if (mode === "free") {
      stroke = [p];
    } else if (mode === "bezier") {
      if (nodes.length >= 2 && near(p, nodes[0])) return finishBezier(true);
      nodes = [...nodes, { x: p.x, y: p.y }];
      placing = nodes.length - 1;
    } else if (mode === "edit") {
      for (let i = 0; i < local.length; i++) {
        const n = local[i];
        for (const part of ["in", "out"] as const) {
          const h = n[part];
          if (h && i === picked && near(p, { x: n.x + h[0], y: n.y + h[1] })) {
            drag = { node: i, part, from: p, start: { ...n } };
            return;
          }
        }
        if (near(p, n)) {
          picked = i;
          drag = { node: i, part: "node", from: p, start: { ...n } };
          return;
        }
      }
      // Off the nodes: editing ends.
      ondone();
    }
  }

  function move(e: PointerEvent) {
    const p = at(e);
    cursor = p;
    if (mode === "free" && stroke.length) {
      stroke = [...stroke, p];
    } else if (mode === "bezier" && placing !== null && pressed) {
      // Dragging out a smooth node: handles on both sides, in line.
      const n = nodes[placing];
      const h: [number, number] = [p.x - n.x, p.y - n.y];
      if (Math.hypot(...h) > 3 * unit) {
        nodes[placing] = { ...n, out: h, in: [-h[0], -h[1]] };
      }
    } else if (mode === "edit" && drag) {
      const { node, part, from, start } = drag;
      const [dx, dy] = [p.x - from.x, p.y - from.y];
      const n = { ...local[node] };
      if (part === "node") {
        n.x = start.x + dx;
        n.y = start.y + dy;
      } else {
        const h = start[part]!;
        const moved: [number, number] = [h[0] + dx, h[1] + dy];
        n[part] = moved;
        // A smooth node keeps its other handle opposite, at its length.
        const other = part === "in" ? "out" : "in";
        const o = start[other];
        if (o && !e.altKey) {
          const len = Math.hypot(...o), m = Math.hypot(...moved) || 1;
          n[other] = [(-moved[0] / m) * len, (-moved[1] / m) * len];
        }
      }
      local[node] = n;
    }
  }

  function up() {
    pressed = false;
    if (mode === "free" && stroke.length) {
      const path = fromStroke(stroke, 2.5 * unit, 14 * unit);
      stroke = [];
      if (path) oncommit(path.nodes, path.closed);
      ondone();
    } else if (mode === "bezier") {
      placing = null;
    } else if (mode === "edit" && drag) {
      drag = null;
      oncommit(local, closed);
    }
  }

  function dbl(e: MouseEvent) {
    if (mode === "bezier") {
      // The double-click's second click added a node: drop it.
      nodes = nodes.slice(0, -1);
      finishBezier(false);
    } else if (mode === "edit") {
      const p = at(e);
      const i = local.findIndex((n) => near(p, n));
      if (i >= 0) {
        local = toggleSmooth(local, i, closed);
        oncommit(local, closed);
      }
    }
  }

  function key(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (mode === "bezier" && (e.key === "Enter" || e.key === "Escape")) {
      e.preventDefault();
      finishBezier(false);
    } else if (mode === "edit" && e.key === "Escape") {
      e.preventDefault();
      ondone();
    } else if (mode === "edit" && (e.key === "Delete" || e.key === "Backspace") && picked !== null && local.length > 2) {
      e.preventDefault();
      e.stopPropagation();
      local = local.filter((_, i) => i !== picked);
      picked = null;
      oncommit(local, closed);
    } else if (mode === "free" && e.key === "Escape") {
      stroke = [];
      ondone();
    }
  }

  const preview = $derived.by(() => {
    if (mode === "free") return stroke.length ? "M" + stroke.map((p) => `${p.x} ${p.y}`).join("L") : "";
    if (mode === "bezier") {
      const shown = cursor && placing === null && nodes.length ? [...nodes, { x: cursor.x, y: cursor.y }] : nodes;
      return pathData(shown, false);
    }
    return pathData(local, closed);
  });
  const shownNodes = $derived(mode === "bezier" ? nodes : mode === "edit" ? local : []);
  const turn = $derived(mode === "edit" && edit?.rotation ? `rotate(${edit.rotation} ${center.x} ${center.y})` : undefined);
</script>

<!-- Moves and the release are followed on the window: pointer capture on SVG
     is not honoured everywhere (WebKitGTK sends them to the element below). -->
<svelte:window onkeydown={key} onpointermove={move} onpointerup={up} onpointercancel={up} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<svg class="path-tool" class:drawing={mode !== "edit"} viewBox="0 0 {width} {height}" preserveAspectRatio="none"
     onpointerdown={down} ondblclick={dbl}>
  <rect class="catch" x="0" y="0" {width} {height} />
  <g transform={turn}>
    <path class="preview" d={preview} stroke-width={1.5 * unit} />
    {#each shownNodes as n, i}
      {#if mode === "edit" && i === picked}
        {#each ["in", "out"] as const as part}
          {@const h = n[part]}
          {#if h}
            <line class="arm" x1={n.x} y1={n.y} x2={n.x + h[0]} y2={n.y + h[1]} stroke-width={unit} />
            <circle class="ctrl" cx={n.x + h[0]} cy={n.y + h[1]} r={4.5 * unit} stroke-width={unit} />
          {/if}
        {/each}
      {/if}
      <rect class="node" class:picked={i === picked} class:first={mode === "bezier" && i === 0 && nodes.length > 1}
            x={n.x - 4.5 * unit} y={n.y - 4.5 * unit} width={9 * unit} height={9 * unit} stroke-width={unit} />
    {/each}
  </g>
</svg>

<style>
  .path-tool { position: absolute; inset: 0; width: 100%; height: 100%; z-index: 5; touch-action: none; }
  .path-tool.drawing { cursor: crosshair; }
  .catch { fill: transparent; }
  .preview { fill: none; stroke: #3f74e0; stroke-dasharray: none; pointer-events: none; }
  .arm { stroke: #3f74e0; pointer-events: none; }
  .ctrl { fill: white; stroke: #3f74e0; cursor: move; }
  .node { fill: white; stroke: #3f74e0; cursor: move; }
  .node.picked { fill: #3f74e0; }
  .node.first { fill: #ffd54f; }
</style>
