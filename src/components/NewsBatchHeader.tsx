export function NewsBatchHeader({
  freshCount,
  cycleEndedAgo,
  onPrevious,
  onNext,
}: {
  freshCount: number;
  cycleEndedAgo: string | null;
  onPrevious?: () => void;
  onNext?: () => void;
}) {
  return (
    <div className="batch-head">
      <span>{freshCount} fresh</span>
      {cycleEndedAgo !== null && (
        <>
          <span className="sep">·</span>
          <span>cycle ended {cycleEndedAgo} ago</span>
        </>
      )}
      <span className="batch-nav">
        <button type="button" aria-label="previous story" onClick={onPrevious}>
          ‹
        </button>
        <button type="button" aria-label="next story" onClick={onNext}>
          ›
        </button>
      </span>
    </div>
  );
}
