import type { Priority } from "../lib/presentation";
import { categoryClass, categoryLabel } from "../lib/presentation";
import { Manifest } from "./Manifest";
import { NewsBatchHeader } from "./NewsBatchHeader";
import { PositionBar } from "./PositionBar";
import { Stamp } from "./Stamp";

export interface NewsStoryView {
  source: string;
  headline: string;
  summary: string;
  category: string | null;
  age: string | null;
  priority: Priority;
  hasLink: boolean;
}

export function NewsBelowBlock({
  stories,
  currentIndex,
  freshCount,
  cycleEndedAgo,
  expanded,
  onPrevious,
  onNext,
}: {
  stories: NewsStoryView[];
  currentIndex: number;
  freshCount: number;
  cycleEndedAgo: string | null;
  expanded: boolean;
  onPrevious?: () => void;
  onNext?: () => void;
}) {
  if (stories.length === 0) {
    return null;
  }
  const clampedIndex = Math.min(Math.max(currentIndex, 0), stories.length - 1);
  const story = stories[clampedIndex];

  return (
    <div
      className={`below-block news-shade ${categoryClass(story.category)}`}
      data-testid="news-below-block"
    >
      <NewsBatchHeader
        freshCount={freshCount}
        cycleEndedAgo={cycleEndedAgo}
        onPrevious={onPrevious}
        onNext={onNext}
      />
      <div className="compact">
        <div className="copy">
          <div className="masthead-row">
            <div className="masthead">
              <span className="dot" />
              {story.source}
            </div>
            <Stamp priority={story.priority} signal="generic" eventType="news_item" />
          </div>
          <div className="title headline">{story.headline}</div>
          {(story.category !== null || story.age !== null) && (
            <div className="notif-meta-row">
              {story.category !== null && (
                <span className="chip chip-category">{categoryLabel(story.category)}</span>
              )}
              {story.age !== null && <span className="notif-time-inline">{story.age}</span>}
            </div>
          )}
        </div>
      </div>
      <Manifest
        title={story.headline}
        body={story.summary}
        eventType="news_item"
        expanded={expanded}
        hasLink={story.hasLink}
      />
      <PositionBar total={stories.length} current={clampedIndex} />
    </div>
  );
}
