import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NowPlayingSummary } from "../useStatusState";
import { MediaBelowBlock } from "./MediaBelowBlock";

afterEach(cleanup);

const CAPTURED_AT_MS = 1_000_000;

function nowPlaying(overrides: Partial<NowPlayingSummary> = {}): NowPlayingSummary {
  return {
    title: "Weightless in Motion",
    artist: "Hiroshi Ando",
    album: "Long Season",
    playing: true,
    elapsedMs: 107_000,
    durationMs: 252_000,
    capturedAtMs: CAPTURED_AT_MS,
    appBundleId: "com.apple.Music",
    ...overrides,
  };
}

describe("MediaBelowBlock (plan 171, slice H)", () => {
  it("renders nothing when media is null", () => {
    const { container } = render(<MediaBelowBlock media={null} />);
    expect(container.querySelector('[data-testid="media-below-block"]')).toBeNull();
  });

  it("carries the below-block class the shipped shell reads from", () => {
    const { container } = render(<MediaBelowBlock media={nowPlaying()} />);
    const block = container.querySelector('[data-testid="media-below-block"]');
    expect(block?.classList.contains("below-block")).toBe(true);
  });

  it("renders the media kicker, title, and artist · album subtitle", () => {
    const { container } = render(<MediaBelowBlock media={nowPlaying()} />);
    expect(container.querySelector(".masthead")?.textContent).toContain("media");
    expect(container.querySelector(".title.headline")?.textContent).toBe("Weightless in Motion");
    expect(container.querySelector(".notif-subtitle")?.textContent).toBe(
      "Hiroshi Ando · Long Season",
    );
  });

  it("falls back to artist alone when there is no album", () => {
    const { container } = render(<MediaBelowBlock media={nowPlaying({ album: null })} />);
    expect(container.querySelector(".notif-subtitle")?.textContent).toBe("Hiroshi Ando");
  });

  it("falls back to album alone when there is no artist", () => {
    const { container } = render(<MediaBelowBlock media={nowPlaying({ artist: null })} />);
    expect(container.querySelector(".notif-subtitle")?.textContent).toBe("Long Season");
  });

  it("renders no subtitle row when neither artist nor album is present", () => {
    const { container } = render(
      <MediaBelowBlock media={nowPlaying({ artist: null, album: null })} />,
    );
    expect(container.querySelector(".notif-subtitle-row")).toBeNull();
  });

  describe("progress bar (reused .media-bar/.media-bar-fill)", () => {
    it("reflects elapsedMs/durationMs as a scaleX fraction", () => {
      const { container } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 63_000, durationMs: 252_000 })} />,
      );
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transform).toBe("scaleX(0.25)");
    });

    it("clamps at 100% when elapsed exceeds duration", () => {
      const { container } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 999_000, durationMs: 252_000 })} />,
      );
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transform).toBe("scaleX(1)");
    });

    it("renders 0% progress when duration is unknown, without dividing by null", () => {
      const { container } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 63_000, durationMs: null })} />,
      );
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transform).toBe("scaleX(0)");
    });

    it("renders the elapsed time via the shared mm:ss formatter", () => {
      const { container } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 107_000, durationMs: 252_000 })} />,
      );
      expect(container.querySelector(".media-time")?.textContent).toBe("1:47");
    });

    // plan 151's discontinuity discipline (the exact pattern this slice's
    // task brief points at, IdleHoverPeek.tsx:300-358), ported faithfully:
    // the CSS `transition: transform 1s linear` (idle-peek.css) is for a
    // steady playback glide only; every discontinuity gets an inline
    // `transition: none` for exactly that render.
    it("snaps (transition: none) on the first render", () => {
      const { container } = render(<MediaBelowBlock media={nowPlaying({ elapsedMs: 30_000 })} />);
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transition).toBe("none");
    });

    it("glides (no inline override) on a steady forward tick between renders", () => {
      const { container, rerender } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 30_000 })} />,
      );
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 31_000 })} />);
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transition).toBe("");
    });

    it("snaps when the transport pauses", () => {
      const { container, rerender } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 30_000 })} />,
      );
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 31_000 })} />);
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 31_000, playing: false })} />);
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transition).toBe("none");
    });

    it("snaps on a track change even when progress moves forward", () => {
      const { container, rerender } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 30_000 })} />,
      );
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 31_000 })} />);
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 90_000, title: "Reunion" })} />);
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transition).toBe("none");
    });

    it("snaps when progress goes backwards (seek)", () => {
      const { container, rerender } = render(
        <MediaBelowBlock media={nowPlaying({ elapsedMs: 90_000 })} />,
      );
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 91_000 })} />);
      rerender(<MediaBelowBlock media={nowPlaying({ elapsedMs: 2_000 })} />);
      const fill = container.querySelector(".media-bar-fill") as HTMLElement;
      expect(fill.style.transition).toBe("none");
    });
  });

  describe("transport buttons (presentational only)", () => {
    it("fires onCommand('previous') on the previous-track button", () => {
      const onCommand = vi.fn();
      render(<MediaBelowBlock media={nowPlaying()} onCommand={onCommand} />);
      fireEvent.click(screen.getByRole("button", { name: "Previous track" }));
      expect(onCommand).toHaveBeenCalledWith("previous");
      expect(onCommand).toHaveBeenCalledTimes(1);
    });

    it("fires onCommand('playPause') on the primary button, labeled Pause while playing", () => {
      const onCommand = vi.fn();
      render(<MediaBelowBlock media={nowPlaying({ playing: true })} onCommand={onCommand} />);
      fireEvent.click(screen.getByRole("button", { name: "Pause" }));
      expect(onCommand).toHaveBeenCalledWith("playPause");
    });

    it("labels the primary button Play while paused, same command either way", () => {
      const onCommand = vi.fn();
      render(<MediaBelowBlock media={nowPlaying({ playing: false })} onCommand={onCommand} />);
      fireEvent.click(screen.getByRole("button", { name: "Play" }));
      expect(onCommand).toHaveBeenCalledWith("playPause");
    });

    it("fires onCommand('next') on the next-track button", () => {
      const onCommand = vi.fn();
      render(<MediaBelowBlock media={nowPlaying()} onCommand={onCommand} />);
      fireEvent.click(screen.getByRole("button", { name: "Next track" }));
      expect(onCommand).toHaveBeenCalledWith("next");
    });

    it("does not throw when onCommand is omitted (a click before any integration wires it up)", () => {
      render(<MediaBelowBlock media={nowPlaying()} />);
      expect(() =>
        fireEvent.click(screen.getByRole("button", { name: "Next track" })),
      ).not.toThrow();
    });
  });

  describe("expanded gates the scrubber and queue preview", () => {
    it("renders no scrubber/queue when expanded is omitted (defaults to false)", () => {
      const { container } = render(<MediaBelowBlock media={nowPlaying()} />);
      expect(container.querySelector(".media-scrub")).toBeNull();
      expect(container.querySelector(".media-times")).toBeNull();
      expect(container.querySelector(".queue-list")).toBeNull();
    });

    it("renders no scrubber/queue when expanded is explicitly false", () => {
      const { container } = render(<MediaBelowBlock media={nowPlaying()} expanded={false} />);
      expect(container.querySelector(".media-scrub")).toBeNull();
      expect(container.querySelector(".queue-list")).toBeNull();
    });

    it("renders the scrubber and duration times when expanded", () => {
      const { container } = render(
        <MediaBelowBlock
          media={nowPlaying({ elapsedMs: 107_000, durationMs: 252_000 })}
          expanded={true}
        />,
      );
      const scrub = container.querySelector(".media-scrub");
      expect(scrub).not.toBeNull();
      const fill = container.querySelector(".media-scrub .fill") as HTMLElement;
      expect(fill.style.width).not.toBe("");
      const times = Array.from(container.querySelectorAll(".media-times span"));
      expect(times.map((s) => s.textContent)).toEqual(["1:47", "4:12"]);
    });

    it("shows a placeholder duration when durationMs is unknown", () => {
      const { container } = render(
        <MediaBelowBlock media={nowPlaying({ durationMs: null })} expanded={true} />,
      );
      const times = Array.from(container.querySelectorAll(".media-times span"));
      expect(times[1]?.textContent).toBe("--:--");
    });

    it("renders no queue-list when expanded but queue is omitted (documented wire gap)", () => {
      const { container } = render(<MediaBelowBlock media={nowPlaying()} expanded={true} />);
      expect(container.querySelector(".queue-list")).toBeNull();
    });

    it("renders no queue-list when expanded but queue is an empty array", () => {
      const { container } = render(
        <MediaBelowBlock media={nowPlaying()} expanded={true} queue={[]} />,
      );
      expect(container.querySelector(".queue-list")).toBeNull();
    });

    it("renders one .queue-row per queue item, in order, when both expanded and queue are given", () => {
      const { container } = render(
        <MediaBelowBlock
          media={nowPlaying()}
          expanded={true}
          queue={[
            { title: "Sable Light", artist: "Hiroshi Ando" },
            { title: "Kite Weather", artist: "Mono No Aware" },
          ]}
        />,
      );
      const rows = Array.from(container.querySelectorAll(".queue-row"));
      expect(rows).toHaveLength(2);
      expect(rows[0].textContent).toContain("Sable Light");
      expect(rows[0].textContent).toContain("Hiroshi Ando");
      expect(rows[0].querySelector(".n")?.textContent).toBe("1");
      expect(rows[1].querySelector(".n")?.textContent).toBe("2");
    });

    it("does not render the queue-list when queue is given but expanded is false", () => {
      const { container } = render(
        <MediaBelowBlock
          media={nowPlaying()}
          expanded={false}
          queue={[{ title: "Sable Light", artist: "Hiroshi Ando" }]}
        />,
      );
      expect(container.querySelector(".queue-list")).toBeNull();
    });
  });
});
