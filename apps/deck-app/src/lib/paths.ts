// Path geometry for the drawing tools: nodes in slide points, Bézier
// handles as offsets from their node (as the backend takes them).

export interface PathNode {
  x: number;
  y: number;
  /** The handle the curve arrives along. */
  in?: [number, number] | null;
  /** The handle the curve leaves along. */
  out?: [number, number] | null;
}

type Pt = { x: number; y: number };

const dist = (a: Pt, b: Pt) => Math.hypot(a.x - b.x, a.y - b.y);

/** Distance from `p` to the segment `a`–`b`. */
function segDist(p: Pt, a: Pt, b: Pt): number {
  const l2 = (b.x - a.x) ** 2 + (b.y - a.y) ** 2;
  if (l2 === 0) return dist(p, a);
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2));
  return dist(p, { x: a.x + t * (b.x - a.x), y: a.y + t * (b.y - a.y) });
}

/** Ramer–Douglas–Peucker: the points that keep the polyline within `tol`. */
export function simplify(points: Pt[], tol: number): Pt[] {
  if (points.length < 3) return points.slice();
  let worst = 0, at = 0;
  const [a, b] = [points[0], points[points.length - 1]];
  for (let i = 1; i < points.length - 1; i++) {
    const d = segDist(points[i], a, b);
    if (d > worst) { worst = d; at = i; }
  }
  if (worst <= tol) return [a, b];
  const left = simplify(points.slice(0, at + 1), tol);
  return left.slice(0, -1).concat(simplify(points.slice(at), tol));
}

/** Smooth Bézier nodes through `points` (Catmull–Rom tangents); an open
 * path's ends are corners. */
export function smoothNodes(points: Pt[], closed: boolean): PathNode[] {
  const n = points.length;
  return points.map((p, i) => {
    const prev = points[closed ? (i - 1 + n) % n : Math.max(i - 1, 0)];
    const next = points[closed ? (i + 1) % n : Math.min(i + 1, n - 1)];
    if (!closed && (i === 0 || i === n - 1)) return { x: p.x, y: p.y };
    const t: [number, number] = [(next.x - prev.x) / 6, (next.y - prev.y) / 6];
    return { x: p.x, y: p.y, in: [-t[0], -t[1]], out: t };
  });
}

/** A freehand stroke as a path: simplified, smoothed, closed when it ends
 * within `snap` of its start. */
export function fromStroke(points: Pt[], tol: number, snap: number): { nodes: PathNode[]; closed: boolean } | null {
  if (points.length < 2) return null;
  const closed = points.length > 4 && dist(points[0], points[points.length - 1]) <= snap;
  let pts = simplify(points, tol);
  if (closed && pts.length > 2) pts = pts.slice(0, -1);
  if (pts.length < 2) return null;
  return { nodes: smoothNodes(pts, closed && pts.length > 2), closed: closed && pts.length > 2 };
}

/** A node turned into a corner (no handles), or a smooth node with
 * handles along its neighbours' direction. */
export function toggleSmooth(nodes: PathNode[], i: number, closed: boolean): PathNode[] {
  const out = nodes.map((n) => ({ ...n }));
  const node = out[i];
  if (node.in || node.out) {
    node.in = null;
    node.out = null;
    return out;
  }
  const n = out.length;
  const prev = out[closed ? (i - 1 + n) % n : Math.max(i - 1, 0)];
  const next = out[closed ? (i + 1) % n : Math.min(i + 1, n - 1)];
  const t: [number, number] = [(next.x - prev.x) / 6, (next.y - prev.y) / 6];
  node.in = [-t[0], -t[1]];
  node.out = t;
  return out;
}

/** SVG path data of nodes in slide points. */
export function pathData(nodes: PathNode[], closed: boolean): string {
  if (!nodes.length) return "";
  const seg = (a: PathNode, b: PathNode) => {
    if (!a.out && !b.in) return `L${b.x} ${b.y}`;
    const o = a.out ?? [0, 0], h = b.in ?? [0, 0];
    return `C${a.x + o[0]} ${a.y + o[1]} ${b.x + h[0]} ${b.y + h[1]} ${b.x} ${b.y}`;
  };
  let d = `M${nodes[0].x} ${nodes[0].y}`;
  for (let i = 1; i < nodes.length; i++) d += seg(nodes[i - 1], nodes[i]);
  if (closed && nodes.length > 1) d += seg(nodes[nodes.length - 1], nodes[0]) + "Z";
  return d;
}

/** Rotates `p` by `deg` degrees about `c`. */
export function rotate(p: Pt, c: Pt, deg: number): Pt {
  if (!deg) return p;
  const a = (deg * Math.PI) / 180, [s, k] = [Math.sin(a), Math.cos(a)];
  const [dx, dy] = [p.x - c.x, p.y - c.y];
  return { x: c.x + dx * k - dy * s, y: c.y + dx * s + dy * k };
}
