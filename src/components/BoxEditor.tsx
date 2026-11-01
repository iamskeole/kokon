import { useEffect, useMemo, useRef, useState } from "react";
import { Crosshair, Square, Trash2, Undo2, X } from "lucide-react";
import { urls } from "../lib/api";
import { inBrowser } from "../lib/platform";
import { focusPrompt, setState, updateComposer, useStore } from "../lib/store";
import { isTyping } from "./Viewer";
import { IconButton } from "./ui";
import { ZoomPan } from "./ZoomPan";
import { parseRatio, type BoxRect, type BoxRow } from "../lib/logic";

const PALETTE = ["#ef4035", "#2f7de1", "#e8a317", "#1f9d6b", "#a24ad8", "#e0569b", "#14a3b8", "#7a8b2e", "#d2691e", "#5562d6"];
const MIN = 0.01;
const KINDS: BoxRow["kind"][] = ["keep", "move", "new", "remove"];
const GRID = Array.from({ length: 9 }, (_, i) => (i + 1) * 100);

const clamp = (v: number) => Math.min(1, Math.max(0, v));
const q = (v: number) => Math.min(1000, Math.max(0, Math.round(v * 1000)));

/** The rect that is drawn and dragged for a row: where a thing is (remove) or where it goes (the rest). */
function mainOf(r: BoxRow): BoxRect {
  return (r.kind === "remove" ? r.src : r.tgt) ?? r.src ?? { x1: 0, y1: 0, x2: 0, y2: 0 };
}

const contains = (r: BoxRect, x: number, y: number) => x >= r.x1 && x <= r.x2 && y >= r.y1 && y <= r.y2;

const clone = (r: BoxRow): BoxRow => ({ ...r, src: r.src ? { ...r.src } : null, tgt: r.tgt ? { ...r.tgt } : null });

export function BoxEditor() {
  const inBrowserHere = inBrowser;
  const setStateHere = setState;
  const updateComposerHere = updateComposer;
  const composer = useStore((s) => s.composer);
  const refs = composer.refs;
  const aspect = composer.aspect;
  const custom = aspect === "custom";
  const [w, h] = [composer.customWidth, composer.customHeight];

  // Existing boxes survive while they point at a reference that still exists.
  const existing = useMemo(
    () => composer.boxes.filter((r) => r.ref == null || r.ref < refs.length),
    [composer.boxes, refs.length],
  );
  const [rows, setRows] = useState<BoxRow[]>(() => existing.map((r) => ({ ...r })));
  const [tab, setTab] = useState<number | null>(() => (refs.length ? 0 : null));
  const [sel, setSel] = useState<number | null>(null);
  const [kind, setKind] = useState<BoxRow["kind"]>("new");
  const [desc, setDesc] = useState("");
  const [draft, setDraft] = useState<BoxRect | null>(null);
  const [hoverMove, setHoverMove] = useState(false);
  const past = useRef<BoxRow[][]>([]);
  const preDrag = useRef<BoxRow[] | null>(null);
  const counter = useRef(Math.max(0, ...existing.map((r) => { const m = /^obj_(\d+)$/.exec(r.id); return m ? +m[1] : 0; })));
  const drag = useRef<
    | { mode: "move"; idx: number; sx: number; sy: number; start: BoxRect; moved: boolean }
    | { mode: "resize"; idx: number; corner: "tl" | "tr" | "bl" | "br"; start: BoxRect; moved: boolean }
    | null
  >(null);
  const svgRef = useRef<SVGSVGElement | null>(null);

  const tabRows = useMemo(() => rows.filter((r) => r.ref === tab), [rows, tab]);
  const selIdx = sel != null && sel < tabRows.length ? sel : null;
  const selRow = selIdx != null ? tabRows[selIdx] : null;
  const color = (i: number) => PALETTE[i % PALETTE.length];
  const selColor = selIdx != null ? color(selIdx) : PALETTE[0];

  // The canvas: the chosen reference, or the blank output shape.
  const ref = tab != null ? refs[tab] : null;
  const size = useMemo(() => {
    if (ref) return { width: ref.width, height: ref.height };
    if (custom) return { width: w, height: h };
    const ratio = parseRatio(aspect) ?? 1;
    return { width: 1024, height: Math.round(1024 / ratio) };
  }, [ref, custom, w, h, aspect]);
  const src = ref ? urls.source(ref.source) : null;
  const preview = ref ? urls.sourcePreview(ref.source) : null;

  const push = (next: BoxRow[]) => {
    past.current.push(rows.map(clone));
    if (past.current.length > 50) past.current.shift();
    setRows(next);
  };
  const undo = () => {
    const prev = past.current.pop();
    if (prev) {
      setRows(prev);
      setSel(null);
      setDesc("");
    }
  };

  const toUnit = (e: { clientX: number; clientY: number }): [number, number] => {
    const el = svgRef.current;
    if (!el) return [0, 0];
    const b = el.getBoundingClientRect();
    return [clamp((e.clientX - b.left) / b.width), clamp((e.clientY - b.top) / b.height)];
  };

  /** Write a new main rect into a row, keeping the kind's invariants (a kept thing is one rect). */
  const setMain = (list: BoxRow[], idx: number, r: BoxRect): BoxRow[] =>
    list.map((row, i) => {
      if (i !== idx) return row;
      const n = { ...row };
      if (n.kind === "remove") n.src = r;
      else if (n.kind === "keep") {
        n.src = { ...r };
        n.tgt = { ...r };
      } else n.tgt = r;
      return n;
    });

  const onDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    const [x, y] = toUnit(e);
    for (let i = tabRows.length - 1; i >= 0; i--) {
      if (contains(mainOf(tabRows[i]), x, y)) {
        const r = tabRows[i];
        setSel(i);
        setDesc(r.desc);
        preDrag.current = rows.map(clone);
        drag.current = { mode: "move", idx: rows.findIndex((v) => v === r), sx: x, sy: y, start: { ...mainOf(r) }, moved: false };
        return;
      }
    }
    drag.current = { mode: "move", idx: -1, sx: x, sy: y, start: { x1: x, y1: y, x2: x, y2: y }, moved: false };
    setDraft({ x1: x, y1: y, x2: x, y2: y });
  };

  const onMove = (e: React.PointerEvent) => {
    const [x, y] = toUnit(e);
    if (!drag.current) {
      const over = tabRows.some((r) => contains(mainOf(r), x, y));
      setHoverMove((v) => (v === over ? v : over));
      return;
    }
    if (drag.current.mode === "move") {
      if (draft) {
        setDraft({ ...draft, x2: x, y2: y });
        return;
      }
      const d = drag.current;
      if (d.idx < 0) return;
      let dx = x - d.sx;
      let dy = y - d.sy;
      if (Math.abs(dx) < 0.002 && Math.abs(dy) < 0.002) return;
      if (!d.moved) {
        d.moved = true;
        if (preDrag.current) {
          past.current.push(preDrag.current);
          if (past.current.length > 50) past.current.shift();
        }
      }
      const s = d.start;
      const nx = clamp(Math.min(1 - (s.x2 - s.x1), Math.max(0, s.x1 + dx)));
      const ny = clamp(Math.min(1 - (s.y2 - s.y1), Math.max(0, s.y1 + dy)));
      setRows((cur) => setMain(cur, d.idx, { ...s, x1: nx, y1: ny }));
      return;
    }
    // resize
    const d = drag.current;
    if (!d.moved) {
      d.moved = true;
      if (preDrag.current) {
        past.current.push(preDrag.current);
        if (past.current.length > 50) past.current.shift();
      }
    }
    const s = d.start;
    let { x1, y1, x2, y2 } = s;
    if (d.corner === "tl" || d.corner === "bl") x1 = Math.min(x, s.x2 - MIN);
    if (d.corner === "tr" || d.corner === "br") x2 = Math.max(x, s.x1 + MIN);
    if (d.corner === "tl" || d.corner === "tr") y1 = Math.min(y, s.y2 - MIN);
    if (d.corner === "bl" || d.corner === "br") y2 = Math.max(y, s.y1 + MIN);
    setRows((cur) => setMain(cur, d.idx, { x1: clamp(x1), y1: clamp(y1), x2: clamp(x2), y2: clamp(y2) }));
  };

  const onUp = () => {
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (draft) {
      const n = {
        x1: Math.min(draft.x1, draft.x2),
        y1: Math.min(draft.y1, draft.y2),
        x2: Math.max(draft.x1, draft.x2),
        y2: Math.max(draft.y1, draft.y2),
      };
      setDraft(null);
      if (n.x2 - n.x1 >= MIN && n.y2 - n.y1 >= MIN) {
        const id = `obj_${counter.current++}`;
        const k: BoxRow["kind"] = tab != null ? kind : "new";
        const row: BoxRow = {
          id,
          ref: tab,
          kind: k,
          src: k === "new" ? null : { ...n },
          tgt: k === "remove" ? null : { ...n },
          desc: "",
        };
        push([...rows, row]);
        setSel(tabRows.length);
      }
      return;
    }
    if (d.moved) preDrag.current = null;
  };

  const changeKind = (k: BoxRow["kind"]) => {
    setKind(k);
    if (selIdx == null) return;
    const r = tabRows[selIdx];
    const main = mainOf(r);
    const n: BoxRow = { ...r, kind: k };
    if (k === "keep") {
      n.src = { ...main };
      n.tgt = { ...main };
    } else if (k === "move") {
      n.src = { ...(r.src ?? main) };
      n.tgt = { ...(r.tgt ?? n.src) };
    } else if (k === "new") {
      n.src = null;
      n.tgt = { ...main };
    } else {
      n.src = { ...main };
      n.tgt = null;
    }
    push(rows.map((x) => (x === r ? n : x)));
  };
  const removeSel = () => {
    if (selIdx == null) return;
    const r = tabRows[selIdx];
    push(rows.filter((x) => x !== r));
    setSel(null);
    setDesc("");
  };
  const done = () => {
    const clean = rows.map((r) => ({ ...r, desc: r.desc.trim() }));
    updateComposerHere({ boxes: clean });
    setStateHere({ overlay: null });
    focusPrompt();
  };

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isTyping(e.target)) {
        setStateHere({ overlay: null });
        return;
      }
      if (isTyping(e.target)) return;
      if ((e.key === "Delete" || e.key === "Backspace") && selIdx != null) {
        e.preventDefault();
        removeSel();
        return;
      }
      if (e.key === "z" && (e.metaKey || e.ctrlKey) && !e.shiftKey) {
        e.preventDefault();
        undo();
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selIdx, tab, kind]);

  const switchTab = (t: number | null) => {
    setTab(t);
    setSel(null);
    setDesc("");
    setHoverMove(false);
  };

  const coords = (r: BoxRect) => `${q(r.x1)},${q(r.y1)}  ${q(r.x2)},${q(r.y2)}`;

  return (
    <div className="overlay editor" onContextMenu={(e) => e.preventDefault()}>
      <div className="overlay-head">
        <h2>Layout boxes</h2>
        <span className="spacer" />
        <IconButton title="Close" onClick={() => setStateHere({ overlay: null })}>
          <X size={17} />
        </IconButton>
      </div>

      <div className="box-tabs">
        <button className={`box-tab ${tab === null ? "on" : ""}`} onClick={() => switchTab(null)} title="Boxes on the new image">
          <span className="box-tab__icon">
            <Square size={12} />
          </span>
          New
        </button>
        {refs.map((r, i) => (
          <button key={r.key} className={`box-tab ${tab === i ? "on" : ""}`} onClick={() => switchTab(i)} title={`Boxes on image ${i + 1}`}>
            <img src={inBrowserHere ? urls.sourcePreview(r.source) : urls.source(r.source)} alt="" />
            <span>{i + 1}</span>
          </button>
        ))}
      </div>

      <div className="editor-stage">
        <ZoomPan
          contentSize={size}
          resetKey={`box-${tab === null ? "new" : tab}`}
          enableToggleZoom={false}
          enablePan
          flat={inBrowser}
          minimapSrc={preview ?? undefined}
        >
          {({ flat }) => (
            <div className={`box-stage ${!ref ? "blank" : ""}`} style={{ width: flat ? "100%" : size.width, height: flat ? "100%" : size.height }}>
              {ref && <img src={src ?? undefined} alt="" draggable={false} style={{ width: "100%", height: "100%", objectFit: "fill" }} />}
              {!ref && !tabRows.length && (
                <div className="box-blank-hint">
                  <Crosshair size={14} style={{ marginRight: 8 }} /> Draw a rectangle where something should go
                </div>
              )}
              <svg
                className="box-svg"
                ref={svgRef}
                viewBox="0 0 1000 1000"
                preserveAspectRatio="none"
                onPointerDown={onDown}
                onPointerMove={onMove}
                onPointerUp={onUp}
                style={{ cursor: hoverMove && !draft ? "move" : "crosshair" }}
              >
                {!ref &&
                  GRID.map((g) => (
                    <g key={g}>
                      <line x1={g} y1={0} x2={g} y2={1000} stroke="currentColor" strokeOpacity={0.07} vectorEffect="non-scaling-stroke" />
                      <line x1={0} y1={g} x2={1000} y2={g} stroke="currentColor" strokeOpacity={0.07} vectorEffect="non-scaling-stroke" />
                    </g>
                  ))}
                {tabRows.map((r, i) => {
                  const c = color(i);
                  const main = mainOf(r);
                  const dim = selIdx != null && selIdx !== i ? 0.4 : 1;
                  const isSel = selIdx === i;
                  const x1 = q(main.x1);
                  const y1 = q(main.y1);
                  const x2 = q(main.x2);
                  const y2 = q(main.y2);
                  return (
                    <g key={r.id} opacity={dim}>
                      {r.kind === "move" && r.src && (
                        <>
                          <line
                            x1={q((r.src.x1 + r.src.x2) / 2)}
                            y1={q((r.src.y1 + r.src.y2) / 2)}
                            x2={q((main.x1 + main.x2) / 2)}
                            y2={q((main.y1 + main.y2) / 2)}
                            stroke={c}
                            strokeWidth={1.5}
                            strokeOpacity={0.85}
                            vectorEffect="non-scaling-stroke"
                          />
                          <rect
                            x={q(r.src.x1)} y={q(r.src.y1)} width={q(r.src.x2) - q(r.src.x1)} height={q(r.src.y2) - q(r.src.y1)}
                            rx={10} fill={c} fillOpacity={0.06} stroke={c} strokeWidth={1.5} strokeDasharray="5 4" vectorEffect="non-scaling-stroke"
                          />
                        </>
                      )}
                      <rect
                        className="box-rect"
                        x={x1} y={y1} width={x2 - x1} height={y2 - y1} rx={10}
                        fill={c}
                        fillOpacity={r.kind === "remove" ? 0.08 : isSel ? 0.3 : 0.16}
                        stroke={c}
                        strokeWidth={isSel ? 2.5 : 2}
                        strokeDasharray={r.kind === "remove" ? "5 4" : undefined}
                        vectorEffect="non-scaling-stroke"
                      />
                      {r.kind === "remove" && (
                        <g stroke={c} strokeWidth={1.5} strokeOpacity={0.9} vectorEffect="non-scaling-stroke">
                          <line x1={x1} y1={y1} x2={x2} y2={y2} />
                          <line x1={x2} y1={y1} x2={x1} y2={y2} />
                        </g>
                      )}
                    </g>
                  );
                })}
                {draft && (
                  <rect
                    x={q(Math.min(draft.x1, draft.x2))} y={q(Math.min(draft.y1, draft.y2))}
                    width={q(Math.abs(draft.x2 - draft.x1))} height={q(Math.abs(draft.y2 - draft.y1))}
                    rx={10} fill="white" fillOpacity={0.12} stroke="white" strokeWidth={1.5} strokeDasharray="5 4" vectorEffect="non-scaling-stroke"
                  />
                )}
              </svg>
              <div className="box-overlay">
                {tabRows.map((r, i) => {
                  const m = mainOf(r);
                  const inside = m.y1 < 0.08;
                  return (
                    <div
                      key={r.id}
                      className={`box-chip ${inside ? "in" : ""}`}
                      style={{ left: `${m.x1 * 100}%`, top: `${m.y1 * 100}%`, ["--c" as string]: color(i), opacity: selIdx != null && selIdx !== i ? 0.45 : 1 }}
                    >
                      <span className="box-chip__id">{r.id}</span>
                      <span className="box-chip__coords">{coords(m)}</span>
                    </div>
                  );
                })}
                {selRow &&
                  (["tl", "tr", "bl", "br"] as const).map((corner) => {
                    const m = mainOf(selRow);
                    const left = corner.includes("l") ? m.x1 : m.x2;
                    const top = corner.includes("t") ? m.y1 : m.y2;
                    return (
                      <div
                        key={corner}
                        className={`box-handle ${corner}`}
                        style={{ left: `${left * 100}%`, top: `${top * 100}%`, ["--c" as string]: selColor }}
                        onPointerDown={(e) => {
                          if (e.button !== 0) return;
                          e.stopPropagation();
                          e.currentTarget.setPointerCapture(e.pointerId);
                          preDrag.current = rows.map(clone);
                          drag.current = { mode: "resize", idx: rows.findIndex((v) => v === selRow), corner, start: { ...m }, moved: false };
                        }}
                        onPointerMove={(e) => {
                          if (drag.current?.mode === "resize") onMove(e);
                        }}
                        onPointerUp={onUp}
                      />
                    );
                  })}
              </div>
              <div className="box-mark tl" />
              <div className="box-mark tr" />
              <div className="box-mark bl" />
              <div className="box-mark br" />
            </div>
          )}
        </ZoomPan>
      </div>

      <div className="editor-tools">
        {selRow && (
          <div className="box-sel" title="the selected box">
            <span className="box-sel__id" style={{ background: selColor }}>{selRow.id}</span>
            <span className="box-sel__coords">{coords(mainOf(selRow))}</span>
          </div>
        )}
        {tab != null && (
          <div className="group">
            {KINDS.map((k) => (
              <button key={k} className={`btn box-kind ${kind === k ? "on" : ""}`} onClick={() => changeKind(k)} title={k === "keep" ? "the thing stays where it is" : k === "move" ? "the thing goes from the dashed rect to this one" : k === "new" ? "a new thing appears here (in the output)" : "the thing goes away"}>
                {k[0].toUpperCase() + k.slice(1)}
              </button>
            ))}
          </div>
        )}
        <input
          className="box-desc"
          value={desc}
          placeholder={selRow ? "Describe this box, in the scene's voice" : "Draw a box, then describe it"}
          disabled={selIdx == null}
          onChange={(e) => {
            const v = e.target.value;
            setDesc(v);
            if (selIdx != null) setRows((cur) => cur.map((r, i) => (i === selIdx ? { ...r, desc: v } : r)));
          }}
          onPointerDown={(e) => e.stopPropagation()}
        />
        <div className="group">
          <IconButton title="Undo" disabled={!past.current.length} onClick={undo}>
            <Undo2 size={17} />
          </IconButton>
          <IconButton title="Remove the selected box" disabled={selIdx == null} onClick={removeSel}>
            <Trash2 size={17} />
          </IconButton>
          <IconButton title="Remove all boxes" disabled={!rows.length} onClick={() => { push([]); setSel(null); setDesc(""); }}>
            <X size={17} />
          </IconButton>
        </div>
        <button className="btn primary" onClick={done}>
          Done
        </button>
      </div>
    </div>
  );
}
