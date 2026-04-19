import { useState } from "react";
import "./styles.css";
import { useSessions } from "./hooks/useSessions";
import { SessionList } from "./components/SessionList";
import { SettingsMenu } from "./components/SettingsMenu";
import { PreviewPane } from "./components/PreviewPane";
import type { Session } from "./types/session";

export default function App() {
  const { sessions, error } = useSessions();
  const [hovered, setHovered] = useState<Session | null>(null);

  return (
    <div className="app split">
      <div className="left">
        <div className="header">
          👁 aieye
          {sessions && <span className="count">{sessions.length}</span>}
        </div>
        {error && <div className="error">{error}</div>}
        {sessions === null && !error && <div className="empty">Scanning…</div>}
        {sessions && (
          <SessionList sessions={sessions} onHover={setHovered} />
        )}
        <SettingsMenu />
      </div>
      <div className="right">
        <PreviewPane session={hovered} />
      </div>
    </div>
  );
}
