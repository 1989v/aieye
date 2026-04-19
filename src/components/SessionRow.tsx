import { useEffect, useRef, useState } from "react";
import type { Session } from "../types/session";
import {
  archiveSessionFile,
  resumeSession,
  resumeSessionForceNew,
  revealInFinder,
} from "../ipc/tauri";

function relativeTime(iso: string): string {
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

function stateDot(state: Session["state"]): string {
  return state === "running" ? "🟢" : state === "recent" ? "🟡" : "🔘";
}

function hostLabel(r: NonNullable<Session["running"]>): string {
  const name =
    r.host_name ??
    (r.host_kind === "terminal"
      ? "Terminal"
      : r.host_kind === "iterm2"
        ? "iTerm"
        : r.host_kind === "vscode"
          ? "VS Code"
          : r.host_kind === "jetbrains"
            ? "JetBrains"
            : "host");
  const shortTty = r.tty.replace("/dev/", "");
  const act =
    r.activity === "generating"
      ? " · generating…"
      : r.activity === "idle"
        ? " · idle"
        : "";
  return `${name} · ${shortTty}${act}`;
}

interface Props {
  session: Session;
  onHover?: (session: Session | null) => void;
}

export function SessionRow({ session, onHover }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  // 바깥 클릭/focus 이동/ESC/panel blur 시 overflow 메뉴 자동 닫힘
  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuOpen(false);
    const onDocClick = (e: MouseEvent) => {
      if (!rootRef.current) return;
      if (!rootRef.current.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onKey);
    window.addEventListener("blur", close);
    return () => {
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", close);
    };
  }, [menuOpen]);

  const onClick = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).dataset.rowAction) return;
    resumeSession(session).catch((err) => console.error(err));
  };

  const preview = session.inline_preview;
  const lastUser = preview?.last_user?.trim();
  const lastAssistant = preview?.last_assistant?.trim();

  return (
    <div
      className="session-row"
      onClick={onClick}
      onMouseEnter={() => onHover?.(session)}
      ref={rootRef}
    >
      <span className="state">{stateDot(session.state)}</span>
      <span className="cli">[{session.cli}]</span>
      <div className="body">
        <div className="title">
          {session.finished && <span className="finished-tick">✓</span>}
          {session.title}
        </div>
        <div className="sub">
          {session.project_path ?? "unknown path"}
          {session.git_branch && <> · {session.git_branch}</>}
          <> · {relativeTime(session.last_activity)}</>
        </div>
        {lastUser && <div className="inline-preview user">❯ {lastUser}</div>}
        {lastAssistant && (
          <div className="inline-preview assistant">↩ {lastAssistant}</div>
        )}
        {session.running && (
          <div className="running-badge">● live · {hostLabel(session.running)}</div>
        )}
      </div>
      <button
        className="row-menu-btn"
        data-row-action="menu"
        onClick={(e) => {
          e.stopPropagation();
          setMenuOpen((o) => !o);
        }}
      >
        ⋯
      </button>
      {menuOpen && (
        <div className="row-menu" onClick={(e) => e.stopPropagation()}>
          <button
            data-row-action="menu"
            onClick={() => {
              revealInFinder(session.jsonl_path).catch((err) => console.error(err));
              setMenuOpen(false);
            }}
          >
            Reveal in Finder
          </button>
          <button
            data-row-action="menu"
            onClick={() => {
              resumeSessionForceNew(session).catch((err) => console.error(err));
              setMenuOpen(false);
            }}
          >
            Open in new terminal
          </button>
          <button
            data-row-action="menu"
            onClick={() => {
              navigator.clipboard.writeText(session.id);
              setMenuOpen(false);
            }}
          >
            Copy session ID
          </button>
          {!session.running && (
            <button
              data-row-action="menu"
              className="danger"
              onClick={() => {
                const ok = window.confirm(
                  `이 세션을 휴지통으로 이동합니다.\n\n[${session.cli}] ${session.title}\n\n목록에서 사라지며 --resume 으로 재개 불가. Finder 휴지통에서 복구 가능.\n\n계속할까요?`,
                );
                if (!ok) return;
                archiveSessionFile(session.jsonl_path).catch((err) =>
                  console.error(err),
                );
                setMenuOpen(false);
              }}
            >
              Move to trash
            </button>
          )}
        </div>
      )}
    </div>
  );
}
