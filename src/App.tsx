import { useSlotState } from "./useSlotState";
import "./styles.css";

function App() {
  const slot = useSlotState();
  const cls =
    slot.state === "empty"
      ? "slot idle"
      : `slot ${slot.priority} ${slot.expanded ? "expanded" : ""}`.trim();

  return (
    <div className={cls}>
      {slot.state === "showing" && (
        <>
          <div className="title">{slot.title}</div>
          <div className="body">{slot.body}</div>
        </>
      )}
    </div>
  );
}

export default App;
