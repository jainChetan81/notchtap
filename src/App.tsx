import { useVisibleNotifications } from "./useVisibleNotifications";
import { usePresentationMode } from "./presentationMode";
import { getMorphShape } from "./morphShape";
import "./styles.css";

function App() {
  const notifications = useVisibleNotifications();
  const mode = usePresentationMode();

  return (
    <div className="stack">
      {notifications.map((n) => {
        const shapeClass = mode === "notch" ? getMorphShape(n.eventType) : "mini";
        return (
          <div
            key={n.id}
            className={`notification ${n.eventType} ${n.phase} ${shapeClass}`.trim()}
          >
            <div className="title">{n.title}</div>
            <div className="body">{n.body}</div>
          </div>
        );
      })}
    </div>
  );
}

export default App;
