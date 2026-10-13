import { useApp } from "../App";
import { Hash } from "../components/common";

export function AuditPage() {
  const { ws } = useApp();
  if (!ws) return null;
  return (
    <div className="page">
      <h1>Audit log</h1>
      <p className="lead">Every transition recorded by the engine, newest first. Persisted locally as an append-only JSON Lines file alongside the workspace.</p>
      <table>
        <thead>
          <tr>
            <th>#</th>
            <th>time (UTC)</th>
            <th>case</th>
            <th>actor</th>
            <th>action</th>
            <th>detail</th>
            <th>H</th>
          </tr>
        </thead>
        <tbody>
          {ws.audit.map((e) => (
            <tr key={e.seq}>
              <td className="mono">{e.seq}</td>
              <td className="mono small">{e.timestamp ?? "—"}</td>
              <td className="mono">{e.scenario ?? "—"}</td>
              <td className="mono">{e.actor}</td>
              <td>{e.action}</td>
              <td className="small">{e.detail}</td>
              <td>
                <Hash hex={e.commitment} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
