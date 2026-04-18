import "./styles.css";
import { useSessions } from "./hooks/useSessions";
import { SessionList } from "./components/SessionList";
import { SettingsMenu } from "./components/SettingsMenu";

export default function App() {
  const { sessions, error } = useSessions();

  return (
    <div className="app">
      <div className="header">
        👁 aieye
        {sessions && <span className="count">{sessions.length}</span>}
      </div>
      {error && <div className="error">{error}</div>}
      {sessions === null && !error && <div className="empty">Scanning…</div>}
      {sessions && <SessionList sessions={sessions} />}
      <SettingsMenu />
    </div>
  );
}
