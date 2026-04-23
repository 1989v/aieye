import { useEffect, useState } from "react";
import type { Session, SessionPreview } from "../types/session";
import { getSessionPreview } from "../ipc/tauri";

interface Props {
  session: Session | null;
}

export function PreviewPane({ session }: Props) {
  const [preview, setPreview] = useState<SessionPreview | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!session) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    getSessionPreview(session.jsonl_path, session.cli)
      .then((p) => {
        if (!cancelled) {
          setPreview(p);
        }
      })
      .catch((err) => {
        console.error(err);
        if (!cancelled) setPreview(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [session?.jsonl_path, session?.cli]);

  if (!session) {
    return (
      <div className="preview-pane empty-preview">
        <div className="hint">Hover a session to preview its conversation.</div>
      </div>
    );
  }

  return (
    <div className="preview-pane">
      <div className="preview-header">
        <div className="preview-title">{session.title}</div>
        <div className="preview-meta">
          [{session.cli}] · {session.project_path ?? "-"}
        </div>
      </div>
      {loading && !preview && <div className="hint">Loading…</div>}
      {preview && preview.recent_turns.length === 0 && (
        <div className="hint">No recent messages.</div>
      )}
      {preview && preview.recent_turns.length > 0 && (
        <div className="turns">
          {preview.recent_turns.map((t, i) => (
            <div key={i} className={`turn ${t.role}`}>
              <div className="turn-role">{t.role === "user" ? "You" : "AI"}</div>
              <div className="turn-text">{t.text}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
