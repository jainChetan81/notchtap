import { useVisibleNotifications } from "./useVisibleNotifications";
import "./styles.css";

function App() {
  const notifications = useVisibleNotifications();

  return (
    <div className="stack">
      {notifications.map((n) => (
        <div key={n.id} className={`notification ${n.phase}`}>
          <div className="title">{n.title}</div>
          <div className="body">{n.body}</div>
        </div>
      ))}
    </div>
  );
}

export default App;
