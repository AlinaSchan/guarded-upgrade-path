import { Fragment, useState } from "react";
import type { CheckResult, EvidenceItem, EvidencePackage } from "../types";
import { Attachments, CLASSIFICATION_DEFINITIONS, Kv, Notice, Pill, Tag, Tags } from "./common";

export function ClassificationLegend() {
  return (
    <div className="grid-4" style={{ margin: "10px 0 16px" }}>
      {(["checked", "simulated", "assumed", "judgment"] as const).map((c) => (
        <div key={c}>
          <Tag c={c} />
          <div className="small muted" style={{ marginTop: 4 }}>
            {CLASSIFICATION_DEFINITIONS[c]}
          </div>
        </div>
      ))}
    </div>
  );
}

function ItemDetail({ item }: { item: EvidenceItem }) {
  return (
    <div style={{ padding: "4px 0 8px" }}>
      <Kv
        rows={[
          ["classification", <Tags cs={item.classifications} />],
          ["concerns", item.concerns === "technical_validity" ? "technical validity" : item.concerns === "desirability" ? "desirability for the protocol" : "both technical validity and desirability"],
          ["evidence / output", <span style={{ whiteSpace: "pre-wrap" }}>{item.evidence}</span>],
          ["method", item.method],
          [
            "not checked",
            item.not_checked.length ? (
              <ul className="plain">
                {item.not_checked.map((n, i) => (
                  <li key={i}>{n}</li>
                ))}
              </ul>
            ) : (
              <span className="muted">nothing further within this row's scope</span>
            ),
          ],
          ["establishes", item.establishes],
          ["remaining uncertainty", item.remaining_uncertainty],
        ]}
      />
      <Attachments items={item.attachments} />
    </div>
  );
}

export function EvidenceTable({ pkg, compact }: { pkg: EvidencePackage; compact?: boolean }) {
  const [open, setOpen] = useState<string | null>(null);
  return (
    <div>
      {!compact && <ClassificationLegend />}
      <table>
        <thead>
          <tr>
            <th style={{ width: 150 }}>row</th>
            <th>claim</th>
            <th style={{ width: 190 }}>known by</th>
          </tr>
        </thead>
        <tbody>
          {pkg.items.map((item) => (
            <Fragment key={item.id}>
              <tr className={`clickable ${item.row === "decision requested" ? "decision" : ""}`} onClick={() => setOpen(open === item.id ? null : item.id)} title="Click to expand method, scope and what was not checked">
                <td className="rowlabel">{item.row}</td>
                <td>{item.claim}</td>
                <td>
                  <Tags cs={item.classifications} />
                </td>
              </tr>
              {open === item.id && (
                <tr>
                  <td colSpan={3} style={{ background: "var(--paper-2)" }}>
                    <ItemDetail item={item} />
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
      {!compact && (
        <div className="grid-2">
          <div className="card quiet">
            <div className="kicker">technically valid?</div>
            <div>{pkg.technical_verdict.statement}</div>
            <div className="small muted" style={{ marginTop: 6 }}>
              checked passed: {pkg.technical_verdict.checked_passed.length} · failed: {pkg.technical_verdict.checked_failed.length} · simulated: {pkg.technical_verdict.simulated.length}
            </div>
          </div>
          <div className="card band">
            <div className="kicker">desirable for the protocol?</div>
            <div>{pkg.desirability}</div>
            <div className="small" style={{ marginTop: 6 }}>
              {pkg.decision_requested}
            </div>
          </div>
        </div>
      )}
      {!compact && (
        <div className="card">
          <div className="kicker">specification boundary — what the checks do not establish</div>
          <ul className="plain">
            {pkg.specification_boundary.map((b, i) => (
              <li key={i}>{b}</li>
            ))}
          </ul>
          <div className="small">
            Claims complete correctness: <Pill kind={pkg.claims_complete_correctness ? "bad" : "neutral"}>{String(pkg.claims_complete_correctness)}</Pill>
          </div>
          <div className="small muted" style={{ marginTop: 6 }}>
            {pkg.simulation_scope}
          </div>
          <div className="small muted">{pkg.fixture_note}</div>
        </div>
      )}
    </div>
  );
}

export function ChecksTable({ checks }: { checks: CheckResult[] }) {
  const [open, setOpen] = useState<string | null>(null);
  if (!checks.length) return <Notice>No checks have been run yet.</Notice>;
  return (
    <table>
      <thead>
        <tr>
          <th>check</th>
          <th style={{ width: 90 }}>result</th>
          <th>output</th>
        </tr>
      </thead>
      <tbody>
        {checks.map((c) => (
          <Fragment key={c.id}>
            <tr className="clickable" onClick={() => setOpen(open === c.id ? null : c.id)}>
              <td>
                <div>{c.name}</div>
                <div className="mono muted">{c.id}</div>
              </td>
              <td>
                <Pill kind={c.passed ? "ok" : "bad"}>{c.passed ? "passed" : "FAILED"}</Pill>
              </td>
              <td className="small">{c.output}</td>
            </tr>
            {open === c.id && (
              <tr>
                <td colSpan={3} style={{ background: "var(--paper-2)" }}>
                  <Kv
                    rows={[
                      ["classification", <Tag c={c.classification} />],
                      ["method", c.method],
                      ["scope", c.scope],
                      [
                        "not covered",
                        c.not_covered.length ? (
                          <ul className="plain">
                            {c.not_covered.map((n, i) => (
                              <li key={i}>{n}</li>
                            ))}
                          </ul>
                        ) : (
                          "—"
                        ),
                      ],
                    ]}
                  />
                </td>
              </tr>
            )}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}
