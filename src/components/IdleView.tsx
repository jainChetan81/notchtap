import { useClock } from "../useClock";

export function IdleView() {
  const { display, dayProgress } = useClock();
  return (
    <div className="idle-view">
      <span className="time">{display}</span>
      <span
        className="timeline"
        style={{ "--day-progress": `${dayProgress}%` } as React.CSSProperties}
        aria-hidden="true"
      />
    </div>
  );
}
