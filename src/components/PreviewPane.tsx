import { useEffect, useRef, useState } from "react";
import type { Session, SessionPreview } from "../types/session";
import { getSessionPreview, openAccessibilitySettings, sendReply, setSettings as saveSettings } from "../ipc/tauri";
import { useSettings } from "../hooks/useSettings";

interface Props {
  session: Session | null;
  focusReplyKey?: number;
  onUnpin?: () => void;
}

type SendState =
  | { kind: "idle" }
  | { kind: "sending" }
  | { kind: "done" }
  | { kind: "permission"; pendingText: string }
  | { kind: "error"; message: string };

function classifyError(raw: string): { prefix: string; rest: string } {
  const m = raw.match(/^([a-z_]+):\s*([\s\S]*)$/i);
  if (m) return { prefix: m[1], rest: m[2] };
  return { prefix: "", rest: raw };
}

function errorHint(prefix: string): string {
  switch (prefix) {
    case "permission":
      return "Open System Settings > Privacy > Accessibility and enable aieye.";
    case "host_unsupported":
      return "Switch reply mode in Settings, or resume the session in a terminal.";
    case "cli_not_found":
      return "Install Claude Code CLI, or switch to Paste mode.";
    case "session_running":
      return "Wait for the response to finish, or switch to Paste mode.";
    case "session_not_running":
      return "Resume this session in a terminal first.";
    case "process_timeout":
      return "Response is still running in background — next refresh will pick it up.";
    default:
      return "Try again, or check the logs.";
  }
}

export function PreviewPane({ session, focusReplyKey, onUnpin }: Props) {
  const [preview, setPreview] = useState<SessionPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [replyText, setReplyText] = useState("");
  const [sendState, setSendState] = useState<SendState>({ kind: "idle" });
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const turnsRef = useRef<HTMLDivElement>(null);
  const { settings, update } = useSettings();

  const switchToHeadless = async () => {
    if (!settings) return;
    const next = { ...settings, reply_mode: "headless" as const };
    update({ reply_mode: "headless" });
    await saveSettings(next);
  };

  useEffect(() => {
    if (!session) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    getSessionPreview(session.jsonl_path, session.cli)
      .then((p) => {
        if (!cancelled) setPreview(p);
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

  useEffect(() => {
    if (focusReplyKey !== undefined) {
      textareaRef.current?.focus();
    }
  }, [focusReplyKey]);

  useEffect(() => {
    if (preview && preview.recent_turns.length > 0 && turnsRef.current) {
      turnsRef.current.scrollTop = turnsRef.current.scrollHeight;
    }
  }, [preview]);

  const doSend = async (text: string) => {
    if (!session) return;
    setSendState({ kind: "sending" });
    try {
      await sendReply(session, text);
      setSendState({ kind: "done" });
      setReplyText("");
      setTimeout(() => setSendState({ kind: "idle" }), 1800);
    } catch (e) {
      const raw = String(e);
      const { prefix } = classifyError(raw);
      if (prefix === "permission") {
        setSendState({ kind: "permission", pendingText: text });
      } else {
        setSendState({
          kind: "error",
          message: `${raw}\n${errorHint(prefix)}`,
        });
      }
    }
  };

  const onSend = async () => {
    const text = replyText.trim();
    if (!text) return;
    await doSend(text);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      onSend();
    } else if (e.key === "Escape") {
      textareaRef.current?.blur();
    }
  };

  if (!session) {
    return (
      <div className="preview-pane empty-preview">
        <div className="hint">Hover a session to preview its conversation.</div>
      </div>
    );
  }

  const sending = sendState.kind === "sending";

  return (
    <div className="preview-pane">
      <div className="preview-header">
        <div className="preview-title">{session.title}</div>
        <div className="preview-meta">
          [{session.cli}] · {session.project_path ?? "-"}
          {onUnpin && (
            <button className="unpin-btn" onClick={onUnpin} title="Unpin">
              ✕
            </button>
          )}
        </div>
      </div>
      {loading && !preview && <div className="hint">Loading…</div>}
      {preview && preview.recent_turns.length === 0 && (
        <div className="hint">No recent messages.</div>
      )}
      {preview && preview.recent_turns.length > 0 && (
        <div className="turns" ref={turnsRef}>
          {preview.recent_turns.map((t, i) => (
            <div key={i} className={`turn ${t.role}`}>
              <div className="turn-role">{t.role === "user" ? "You" : "AI"}</div>
              <div className="turn-text">{t.text}</div>
            </div>
          ))}
        </div>
      )}
      <div className="reply-box">
        <textarea
          ref={textareaRef}
          className="reply-textarea"
          placeholder="Reply… (⌘↵ to send, Shift+↵ for newline)"
          value={replyText}
          onChange={(e) => setReplyText(e.target.value)}
          onKeyDown={onKeyDown}
          disabled={sending}
          rows={3}
        />
        <div className="reply-actions">
          <span className={`reply-status ${sendState.kind}`}>
            {sendState.kind === "sending" && "Sending…"}
            {sendState.kind === "done" && "Sent ✓"}
            {sendState.kind === "error" && <pre>{sendState.message}</pre>}
            {sendState.kind === "permission" && (
              <div className="reply-permission">
                <div>
                  aieye needs Accessibility permission to paste into the terminal.
                </div>
                <div className="reply-permission-hint">
                  Tip: aieye is ad-hoc signed during development, so System Settings may not recognize the latest build. If you already enabled aieye but still see this error, <strong>remove aieye from the Accessibility list and add it again</strong>, then quit and relaunch aieye.
                </div>
                <div className="reply-permission-hint">
                  Alternatives — try iTerm2 (only one-time Automation prompt, no Accessibility), or use <strong>Headless mode</strong> which bypasses permissions entirely. Note: Claude Code headless usage may be separately billed from mid-2026.
                </div>
                <div className="reply-permission-actions">
                  <button
                    type="button"
                    onClick={() => {
                      openAccessibilitySettings().catch((err) => console.error(err));
                    }}
                  >
                    Open System Settings
                  </button>
                  {settings && settings.reply_mode !== "headless" && (
                    <button
                      type="button"
                      onClick={async () => {
                        if (sendState.kind !== "permission") return;
                        const pending = sendState.pendingText;
                        await switchToHeadless();
                        await doSend(pending);
                      }}
                    >
                      Switch to Headless & retry
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => {
                      if (sendState.kind === "permission") {
                        doSend(sendState.pendingText);
                      }
                    }}
                  >
                    Retry
                  </button>
                </div>
              </div>
            )}
          </span>
          <button
            className="reply-send"
            onClick={onSend}
            disabled={sending || !replyText.trim()}
          >
            Send ⌘↵
          </button>
        </div>
      </div>
    </div>
  );
}
