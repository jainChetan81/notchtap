import { Fragment, type ReactNode } from "react";

type SegmentKind = "text" | "code" | "bold" | "italic";

type Segment = {
  kind: SegmentKind;
  text: string;
};

const INLINE_PATTERNS: ReadonlyArray<{ kind: SegmentKind; pattern: RegExp }> = [
  { kind: "code", pattern: /`([^`]+)`/ },
  { kind: "bold", pattern: /\*\*([^*]+)\*\*/ },
  { kind: "italic", pattern: /\*([^*]+)\*/ },
];

function tokenizeLine(line: string): Segment[] {
  let segments: Segment[] = [{ kind: "text", text: line }];
  for (const { kind, pattern } of INLINE_PATTERNS) {
    const next: Segment[] = [];
    for (const segment of segments) {
      if (segment.kind !== "text") {
        next.push(segment);
        continue;
      }
      let rest = segment.text;
      let match = pattern.exec(rest);
      while (match !== null) {
        const before = rest.slice(0, match.index);
        if (before !== "") {
          next.push({ kind: "text", text: before });
        }
        next.push({ kind, text: match[1] });
        rest = rest.slice(match.index + match[0].length);
        match = pattern.exec(rest);
      }
      if (rest !== "") {
        next.push({ kind: "text", text: rest });
      }
    }
    segments = next;
  }
  return segments;
}

export function renderInlineMarkdown(text: string): ReactNode {
  const lines = text.split(/\r\n|\r|\n/);
  return lines.map((line, lineIndex) => (
    // biome-ignore lint/suspicious/noArrayIndexKey: the line/segment positions are the stable identity here — the input is derived text that never reorders between renders.
    <Fragment key={`line-${lineIndex}`}>
      {tokenizeLine(line).map((segment, segmentIndex) => {
        const key = `seg-${lineIndex}-${segmentIndex}`;
        switch (segment.kind) {
          case "code":
            return <code key={key}>{segment.text}</code>;
          case "bold":
            return <strong key={key}>{segment.text}</strong>;
          case "italic":
            return <em key={key}>{segment.text}</em>;
          default:
            return <Fragment key={key}>{segment.text}</Fragment>;
        }
      })}
      {lineIndex < lines.length - 1 && <br />}
    </Fragment>
  ));
}
