import { Check, Trash2, Undo2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { urls } from "../lib/api";
import { parseRatio, type BoxRow } from "../lib/logic";
import { inBrowser } from "../lib/platform";
import { focusPrompt, setState, updateComposer, useStore } from "../lib/store";
import { IconButton } from "./ui";
import { isTyping } from "./Viewer";
import { ZoomPan } from "./ZoomPan";

/** The ten box colours, in order (one per row, wrapping). */
const PALETTE = ["#f59e0b", "#ef4444", "#22c55e", "#3b82f6", "#a855f7", "#ec4899", "#14b8a6", "#eab308", "#6366f1", "#84cc16"];
const MIN = 0.01; // a drawn box smaller than this (in canvas fractions) is a click, not a box

type Kind = BoxRow["kind"];

/** Draw rectangles to tell the model where to place, keep or remove things. The boxes travel in
 * the prompt as a JSON table the model reads (BFL's layout-box contract) - klein only. */
export function BoxEditor() {
  const refs = useStore((s) => s.composer.refs);
  const existing = useStore((s) => s.composer.boxes);
  const composer = useStore((s) => s.composer);
  const ref0 = refs[0];
  // With references the boxes sit on the first image (ref_image_0); without, on a blank
  // canvas at the output shape.
  const edit = !!ref0;
  const [rows, setRows] = useState<BoxRow[]>(() => existing.map((r) => ({ ...r })));
  const [sel, setSel] = useState<number | null>(null);
  const [kind, setKind] = useState<Kind>("new");
  const [desc, setDesc] = useState("");
  const [draft, setDraft] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const past = useRef<BoxRow[][]>([]);
  const preDrag = useRef<BoxRow[] | null>(null);
  const counter = useRef<number>(existing.reduce((m, r) => {
    const n = Number(r.id.replace(/\D/g, ""));
    return Number.isFinite(n) && n > m ? n : m;
  }, 0));
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<{ idx: number; sx: number; sy: number; ox: number; oy: number; moved: boolean } | null>(null);

  // Canvas size in content px: the reference's shape, or the output shape (aspect or custom).
  const size = useMemo(() => {
    if (ref0) return { width: ref0.width, height: ref0.height };
    if (composer.resolution === "custom") return { width: composer.customWidth, height: composer.customHeight };
    const ratio = composer.aspect === "auto" ? 1 : (parseRatio(composer.aspect) ?? 1);
    return { width: 1024, height: Math.round(1024 / ratio) };
  }, [ref0, composer]);

  const src = ref0 ? urls.source(ref0.source) : "";
  const preview = ref0 && inBrowser ? urls.sourcePreview(ref0.source) : undefined;

  const push = (next: BoxRow[]) => {
    past.current.push(rows.map((r) => ({ ...r })));
    if (past.current.length > 50) past.current.shift();
    setRows(next);
  };
  const undo = () => {
    const prev = past.current.pop();
    if (prev) {
      setRows(prev.map((r) => ({ ...r })));
      setSel(null);
      setDesc("");
    }
  };

  /** Pointer position in 0-1 canvas fractions (the on-screen box does the zoom math). */
  const toUnit = (e: React.PointerEvent): [number, number] => {
    const r = svgRef.current?.getBoundingClientRect();
    if (!r || r.width <= 0 || r.height <= 0) return [0, 0];
    return [
      Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)),
      Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)),
    ];
  };

  const onDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    try {
      (e.target as Element).setPointerCapture(e.pointerId);
    } catch {}
    const [x, y] = toUnit(e);
    const idx = rows.findIndex((r) => x >= r.x1 && x <= r.x2 && y >= r.y1 && y <= r.y2);
    if (idx >= 0) {
      setSel(idx);
      setDesc(rows[idx].desc);
      preDrag.current = rows.map((r) => ({ ...r }));
      drag.current = { idx, sx: x, sy: y, ox: rows[idx].x1, oy: rows[idx].y1, moved: false };
    } else {
      setSel(null);
      setDesc("");
      setDraft({ x1: x, y1: y, x2: x, y2: y });
    }
  };

  const onMove = (e: React.PointerEvent) => {
    if (draft) {
      const [x, y] = toUnit(e);
      setDraft((d) => (d ? { ...d, x2: x, y2: y } : d));
      return;
    }
    const d = drag.current;
    if (!d) return;
    const [x, y] = toUnit(e);
    const dx = x - d.sx;
    const dy = y - d.sy;
    if (Math.abs(dx) > 0.002 || Math.abs(dy) > 0.002) d.moved = true;
    if (!d.moved) return;
    const next = rows.map((r, i) => {
      if (i !== d.idx) return r;
      const w = r.x2 - r.x1;
      const h = r.y2 - r.y1;
      let x1 = d.ox + dx;
      let y1 = d.oy + dy;
      x1 = Math.min(1 - w, Math.max(0, x1));
      y1 = Math.min(1 - h, Math.max(0, y1));
      return { ...r, x1, y1, x2: x1 + w, y2: y1 + h };
    });
    setRows(next);
  };

  const onUp = () => {
    if (draft) {
      const n = normalize(draft);
      setDraft(null);
      if (n) {
        const row: BoxRow = {
          id: `obj_${++counter.current}`,
          kind: edit ? kind : "new",
          x1: n.x1, y1: n.y1, x2: n.x2, y2: n.y2,
          desc: "",
        };
        push([...rows, row]);
        setSel(rows.length);
      }
      preDrag.current = null;
      return;
    }
    const d = drag.current;
    drag.current = null;
    if (d?.moved && preDrag.current) past.current.push(preDrag.current);
    preDrag.current = null;
  };

  const removeSel = () => {
    if (sel == null) return;
    push(rows.filter((_, i) => i !== sel));
    setSel(null);
    setDesc("");
  };
  const clearAll = () => {
    if (!rows.length) return;
    push([]);
    setSel(null);
    setDesc("");
  };
  const done = () => {
    updateComposer({ boxes: rows.map((r) => ({ ...r, desc: r.desc.trim() })) });
    setState({ overlay: null });
    focusPrompt();
  };

  // Keyboard: Escape cancels, Del removes the selected box, Ctrl+Z undoes.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (isTyping(e.target)) return;
      if (e.key === "Escape") {
        setState({ overlay: null });
      } else if ((e.key === "Delete" || e.key === "Backspace") && sel != null) {
        e.preventDefault();
        removeSel();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        undo();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel, rows]);

  const setDescLive = (v: string) => {
    setDesc(v);
    if (sel != null) setRows((rs) => rs.map((r, i) => (i === sel ? { ...r, desc: v } : r)));
  };

  const selected = sel != null ? rows[sel] : null;

  return (
    <div className="overlay">
      <div className="overlay-head">
        <h2>Layout boxes</h2>
        <span className="spacer" />
        <IconButton title="Cancel (Esc)" onClick={() => setState({ overlay: null })}>
          <X size={18} />
        </IconButton>
      </div>

      <div className="editor-stage">
        <ZoomPan
          contentSize={size}
          resetKey={src || "blank"}
          enableToggleZoom={false}
          enablePan
          className="box-zoom-viewport"
          minimapSrc={preview || undefined}
          flat={inBrowser}
        >
          {({ flat, wantsFull }) => (
            <div
              className={`box-stage ${edit ? "" : "blank"}`}
              style={{ width: flat ? "100%" : size.width, height: flat ? "100%" : size.height }}
              onContextMenu={(e) => e.preventDefault()}
            >
              {edit ? (
                <img
                  src={preview && !wantsFull ? preview : src}
                  alt=""
                  draggable={false}
                  style={{ width: "100%", height: "100%", display: "block", objectFit: "fill" }}
                />
              ) : (
                <div className="box-blank-hint">Draw a rectangle where something should go</div>
              )}
              <svg
                ref={svgRef}
                className="box-svg"
                viewBox="0 0 1000 1000"
                preserveAspectRatio="none"
                style={{ position: "absolute", inset: 0, width: "100%", height: "100%", cursor: "crosshair" }}
                onPointerDown={onDown}
                onPointerMove={onMove}
                onPointerUp={onUp}
                onPointerCancel={onUp}
              >
                {rows.map((r, i) => {
                  const c = PALETTE[i % PALETTE.length];
                  const s = sel === i;
                  return (
                    <g key={r.id}>
                      <rect
                        x={r.x1 * 1000} y={r.y1 * 1000}
                        width={Math.max(0, (r.x2 - r.x1) * 1000)} height={Math.max(0, (r.y2 - r.y1) * 1000)}
                        fill={c} fillOpacity={s ? 0.26 : 0.16}
                        stroke={c} strokeWidth={s ? 4 : 2} vectorEffect="non-scaling-stroke"
                        className="box-rect"
                      />
                      <text x={r.x1 * 1000 + 6} y={r.y1 * 1000 + 16} fontSize={13} fill={c} stroke="#111" strokeWidth={3} paintOrder="stroke">
                        {r.id}
                      </text>
                    </g>
                  );
                })}
                {draft && normalize(draft) && (
                  <rect
                    x={Math.min(draft.x1, draft.x2) * 1000} y={Math.min(draft.y1, draft.y2) * 1000}
                    width={Math.abs(draft.x2 - draft.x1) * 1000} height={Math.abs(draft.y2 - draft.y1) * 1000}
                    fill="#fff" fillOpacity={0.08} stroke="#fff" strokeWidth={1.5} strokeDasharray="6 4" vectorEffect="non-scaling-stroke"
                  />
                )}
              </svg>
            </div>
          )}
        </ZoomPan>
      </div>

      <div className="editor-tools">
        {edit && (
          <div className="group">
            {(["keep", "new", "remove"] as Kind[]).map((k) => (
              <button key={k} className={`btn box-kind ${kind === k ? "on" : ""}`} onClick={() => setKind(k)} title={
                k === "keep" ? "Keep: the thing inside the box stays where it is"
                : k === "new" ? "New: put the described thing inside the box"
                : "Remove: take the thing inside the box away"}>
                {k === "keep" ? "Keep" : k === "new" ? "New" : "Remove"}
              </button>
            ))}
          </div>
        )}
        <input
          className="box-desc"
          type="text"
          placeholder={selected ? `Describe ${selected.id}` : "Select a box to describe it"}
          value={desc}
          disabled={sel == null}
          onChange={(e) => setDescLive(e.target.value)}
        />
        <div className="group">
          <IconButton title="Undo (Ctrl+Z)" onClick={undo} disabled={!past.current.length}>
            <Undo2 size={17} />
          </IconButton>
          <IconButton title="Remove selected (Del)" onClick={removeSel} disabled={sel == null}>
            <Trash2 size={17} />
          </IconButton>
          <IconButton title="Remove all boxes" onClick={clearAll} disabled={!rows.length}>
            <X size={17} />
          </IconButton>
        </div>
        <button className="btn primary" onClick={done}>
          <Check size={15} /> Done
        </button>
      </div>
    </div>
  );
}

/** The drawn rectangle, normalised (x1 < x2, y1 < y2); null if too small to be a box. */
function normalize(d: { x1: number; y1: number; x2: number; y2: number }) {
  const x1 = Math.min(d.x1, d.x2);
  const y1 = Math.min(d.y1, d.y2);
  const x2 = Math.max(d.x1, d.x2);
  const y2 = Math.max(d.y1, d.y2);
  if (x2 - x1 < MIN || y2 - y1 < MIN) return null;
  return { x1, y1, x2, y2 };
}
