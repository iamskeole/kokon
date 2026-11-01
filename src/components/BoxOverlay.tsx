import type { BoxTableRow } from "../lib/logic";

/** The model's own box colours (same order, so a box keeps its hue between editor and result). */
const COLORS = ["#ef4035", "#2f7de1", "#e8a317", "#1f9d6b", "#a24ad8", "#e0569b", "#14a3b8", "#7a8b2e", "#d2691e", "#5562d6"];

/**
 * The layout boxes a stored prompt carried, drawn over the image they apply to.
 * `on: "result"` draws the boxes that act on the output (target boxes, plus the
 * plain placements of a text-to-image prompt); `on: N` draws the source boxes the
 * Nth reference image carried. The wire's 0-1000 canvas scaled to the image, so it
 * lines up exactly; it never catches the pointer.
 */
export function BoxOverlay({ rows, on }: { rows: BoxTableRow[]; on: "result" | number }) {
  const items: { id: string; x: number; y: number; w: number; h: number; c: string }[] = [];
  rows.forEach((r, i) => {
    let b: [number, number, number, number] | null = null;
    if (on === "result") b = r.tgt_bbox ?? r.bbox ?? null;
    else b = r.from === `ref_image_${on}` ? r.src_bbox ?? null : null;
    if (!b) return;
    const [y1, x1, y2, x2] = b;
    if (y2 <= y1 || x2 <= x1) return;
    items.push({ id: r.id, x: x1, y: y1, w: x2 - x1, h: y2 - y1, c: COLORS[i % COLORS.length] });
  });
  if (!items.length) return null;
  return (
    <svg className="box-overlay" viewBox="0 0 1000 1000" preserveAspectRatio="none" aria-hidden="true">
      {items.map((it) => (
        <g key={it.id}>
          <rect
            x={it.x}
            y={it.y}
            width={it.w}
            height={it.h}
            rx={12}
            fill={it.c}
            fillOpacity={0.09}
            stroke={it.c}
            strokeWidth={2.5}
            vectorEffect="non-scaling-stroke"
          />
          <text
            x={it.x + 10}
            y={it.y + 32}
            fill={it.c}
            stroke="#101010"
            strokeWidth={4}
            paintOrder="stroke"
            fontSize={26}
            fontWeight={650}
            style={{ fontFamily: "var(--mono)" }}
          >
            {it.id}
          </text>
        </g>
      ))}
    </svg>
  );
}
