import type { ReactNode } from "react";
import type { Classification } from "../types";

export const CLASSIFICATION_DEFINITIONS: Record<Classification, string> = {
  checked: "Deterministic: a proof, a static check or an exact query.",
  simulated: "Seen on a fork or replay; holds for the cases run.",
  assumed: "Taken as given; nothing in the package checks it.",
  judgment: "Left to the guardians. Evidence informs it, never settles it.",
};

export function Tag({ c }: { c: Classification }) {
  return (
    <span className={`tag ${c}`} title={CLASSIFICATION_DEFINITIONS[c]}>
      {c}
    </span>
  );
}

export function Tags({ cs }: { cs: Classification[] }) {
  return (
    <>
      {cs.map((c) => (
        <Tag key={c} c={c} />
      ))}
    </>
  );
}

export function Pill({ kind, children, title }: { kind?: "ok" | "bad" | "neutral" | "ochre"; children: ReactNode; title?: string }) {
  return (
    <span className={`tag ${kind ?? "neutral"}`} title={title}>
      {children}
    </span>
  );
}

export function shorten(hex: string | null | undefined): string {
  if (!hex) return "—";
  const h = hex.startsWith("0x") ? hex.slice(2) : hex;
  return h.length > 12 ? `0x${h.slice(0, 4)}…${h.slice(-4)}` : hex;
}

export function Hash({ hex, full }: { hex: string | null | undefined; full?: boolean }) {
  if (!hex) return <span className="muted">—</span>;
  return (
    <span className="hash" title={hex}>
      {full ? hex : shorten(hex)}
    </span>
  );
}

export function OpenQuestion({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <div className="open-question">
      <div className="label">{label ?? "Open question"}</div>
      <div>{children}</div>
    </div>
  );
}

export function Notice({ kind, children }: { kind?: "ok" | "bad" | "ochre" | "plain"; children: ReactNode }) {
  return <div className={`notice ${kind && kind !== "plain" ? kind : ""}`}>{children}</div>;
}

export function Kv({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <div className="kv">
      {rows.map(([k, v], i) => (
        <div key={i} style={{ display: "contents" }}>
          <div className="k">{k}</div>
          <div>{v}</div>
        </div>
      ))}
    </div>
  );
}

export function Diff({ text }: { text: string }) {
  return (
    <pre className="diff">
      {text.split("\n").map((line, i) => (
        <div key={i} className={line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : ""}>
          {line}
        </div>
      ))}
    </pre>
  );
}

export function fmt(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  return n.toLocaleString("en-US");
}

export function pct(bps: number): string {
  return `${(bps / 100).toFixed(2)}%`;
}

export function Section({ title, kicker, children }: { title: string; kicker?: string; children: ReactNode }) {
  return (
    <section>
      {kicker && <div className="kicker">{kicker}</div>}
      <h2>{title}</h2>
      {children}
    </section>
  );
}

export function Attachments({ items }: { items: { kind: string; title: string; body: string }[] }) {
  if (!items.length) return null;
  return (
    <div>
      {items.map((a, i) => (
        <details key={i}>
          <summary>
            <span className="mono">{a.kind}</span> · {a.title}
          </summary>
          <pre>{a.body}</pre>
        </details>
      ))}
    </div>
  );
}
