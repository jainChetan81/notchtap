import { MotionConfig } from "motion/react";
import { useSlotState } from "./useSlotState";
import { StatusRailCard } from "./components/StatusRailCard";
import "./styles.css";

function App() {
  const slot = useSlotState();
  return (
    <MotionConfig reducedMotion="user">
      <StatusRailCard slot={slot} />
    </MotionConfig>
  );
}

export default App;
