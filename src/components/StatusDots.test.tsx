import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { StatusState } from "../useStatusState";
import { StatusDots } from "./StatusDots";

afterEach(cleanup);

const ALL_ON: StatusState = {
	paused: false,
	waiting: 3,
	agent: { activeSessions: 0 },
	football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
	news: { enabled: true, chargeFraction: 0, chargeCount: 0, isCharged: false }
};

const ALL_OFF: StatusState = {
	paused: false,
	waiting: 0,
	agent: { activeSessions: 0 },
	football: { enabled: false, live: null },
	news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false }
};

describe("StatusDots", () => {
	describe("accessible names + non-color shapes", () => {
		it("names every dot 'enabled' and shapes it filled when every source is enabled", () => {
			render(<StatusDots status={ALL_ON} />);
			const football = screen.getByRole("img", { name: "Football — enabled" });
			const news = screen.getByRole("img", { name: "News — enabled" });
			for (const dot of [football, news]) {
				expect(dot.classList.contains("shape-enabled")).toBe(true);
			}
		});

		it("names every dot 'disabled' and shapes it hollow when every source is disabled", () => {
			render(<StatusDots status={ALL_OFF} />);
			const football = screen.getByRole("img", { name: "Football — disabled" });
			const news = screen.getByRole("img", { name: "News — disabled" });
			for (const dot of [football, news]) {
				expect(dot.classList.contains("shape-disabled")).toBe(true);
			}
		});

		it("names every dot 'status unavailable' and shapes it hollow-circle when status is omitted", () => {
			render(<StatusDots />);
			const football = screen.getByRole("img", { name: "Football — status unavailable" });
			const news = screen.getByRole("img", { name: "News — status unavailable" });
			for (const dot of [football, news]) {
				expect(dot.classList.contains("shape-unavailable")).toBe(true);
			}
		});

		it("reads each dot's name/shape off its own source independently", () => {
			render(
				<StatusDots
					status={{
						paused: false,
						waiting: 0,
						agent: { activeSessions: 0 },
						football: { enabled: true, live: null },
						news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false }
					}}
				/>
			);
			expect(screen.getByRole("img", { name: "Football — enabled" })).toBeTruthy();
			expect(screen.getByRole("img", { name: "News — disabled" })).toBeTruthy();
		});

		it("keeps a dot labeled 'enabled' (dim, configured shape retained) while paused — the pause fact lives only on the glyph", () => {
			const { container } = render(
				<StatusDots
					status={{
						paused: true,
						waiting: 0,
						agent: { activeSessions: 0 },
						football: { enabled: true, live: null },
						news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false }
					}}
				/>
			);
			const football = screen.getByRole("img", { name: "Football — enabled" });
			expect(football.classList.contains("dim")).toBe(true);
			expect(football.classList.contains("active")).toBe(false);
			expect(football.classList.contains("shape-enabled")).toBe(true);

			const glyph = screen.getByRole("img", { name: "Notifications paused" });
			expect(glyph.classList.contains("pause-glyph")).toBe(true);
			expect(container.querySelectorAll('[aria-label="Notifications paused"]')).toHaveLength(1);

			for (const dot of Array.from(container.querySelectorAll(".status-dot"))) {
				expect(dot.getAttribute("aria-label")?.toLowerCase()).not.toContain("paused");
			}
		});
	});

	describe("overlay-card.css string pins", () => {
		function readSourceCss(relativePath: string): string {
			const url = new NodeURL(relativePath, import.meta.url);
			const raw = readFileSync(fileURLToPath(url), "utf-8");
			return raw.replace(/^@import\s+["'](\.[^"']+)["'];\s*$/gm, (_match, importPath: string) =>
				readFileSync(fileURLToPath(new NodeURL(importPath, url)), "utf-8")
			);
		}

		function ruleBody(css: string, selector: string): string {
			const marker = `${selector} {`;
			const start = css.indexOf(marker);
			if (start === -1) {
				throw new Error(`selector not found in stylesheet: ${selector}`);
			}
			const braceStart = start + marker.length - 1;
			const braceEnd = css.indexOf("}", braceStart);
			if (braceEnd === -1) {
				throw new Error(`unterminated rule for selector: ${selector}`);
			}
			return css.slice(braceStart + 1, braceEnd);
		}

		const overlayCardCss = readSourceCss("../overlay-card.css");

		it(".status-dot's transition softens border-radius AND (post-C2) background-color, not just opacity/box-shadow", () => {
			const body = ruleBody(overlayCardCss, ".card-root .status-dot");
			expect(body).toContain("border-radius var(--hover-ms");
			expect(body).toContain("background-color var(--hover-ms");
			expect(body).toContain("border-color var(--hover-ms");
			expect(body).toContain("border-width var(--hover-ms");
		});

		it("the pause glyph's fade-in keyframe exists, and .pause-glyph references it by exact name", () => {
			expect(overlayCardCss).toContain("@keyframes pause-glyph-fade-in");
			const body = ruleBody(overlayCardCss, ".card-root .pause-glyph");
			expect(body).toContain("pause-glyph-fade-in");
		});
	});
});
