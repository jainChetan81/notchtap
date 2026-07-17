import { useSlotState } from "./useSlotState";
import "./styles.css";

function App() {
  const slot = useSlotState();
  // no exit animation in this pass — slot.state flips straight to null on
  // rotation-out; see docs/V3_6_TECHNICAL_SPEC.md §5.3 for the open gap
  // this resolves (enter-only, via the mount-keyed CSS animation)
  if (slot.state === "empty") return null;
  const cls = `slot ${slot.priority} ${slot.expanded ? "expanded" : ""}`.trim();
  return (
    <div key={slot.id} className={cls}>
      <div className="title">{slot.title}</div>
      <div className="body">{slot.body}</div>
    </div>
  );
}

export default App;
