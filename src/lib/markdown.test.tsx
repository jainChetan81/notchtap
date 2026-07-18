import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { renderInlineMarkdown } from "./markdown";

// same jsdom cleanup gap as StatusRailCard.test.tsx — this project's
// vitest config doesn't set test.globals, so RTL's auto-cleanup never
// registers; without this, containers leak across tests.
afterEach(cleanup);

function renderMarkdown(text: string): HTMLElement {
  const { container } = render(<div>{renderInlineMarkdown(text)}</div>);
  return container;
}

describe("renderInlineMarkdown", () => {
  it("leaves plain text untouched (no elements, same visible text)", () => {
    const container = renderMarkdown("Workspace command is waiting");
    expect(container.textContent).toBe("Workspace command is waiting");
    expect(container.querySelector("code, strong, em, br")).toBeNull();
  });

  it("renders a `code` span as a <code> element", () => {
    const container = renderMarkdown("run `git push` now");
    const code = container.querySelector("code");
    expect(code?.textContent).toBe("git push");
    expect(container.textContent).toBe("run git push now");
  });

  it("renders **bold** as <strong> and *italic* as <em>", () => {
    const container = renderMarkdown("**urgent** and *maybe*");
    expect(container.querySelector("strong")?.textContent).toBe("urgent");
    expect(container.querySelector("em")?.textContent).toBe("maybe");
    expect(container.textContent).toBe("urgent and maybe");
  });

  it("renders adjacent tokens side by side", () => {
    const container = renderMarkdown("`a`**b***c*");
    expect(container.querySelector("code")?.textContent).toBe("a");
    expect(container.querySelector("strong")?.textContent).toBe("b");
    expect(container.querySelector("em")?.textContent).toBe("c");
    expect(container.textContent).toBe("abc");
  });

  it("renders unclosed markers literally", () => {
    const container = renderMarkdown("a **bold and `code");
    expect(container.querySelector("strong, code")).toBeNull();
    expect(container.textContent).toBe("a **bold and `code");
  });

  it("renders <script>alert(1)</script> as visible text, never an element", () => {
    const container = renderMarkdown("<script>alert(1)</script>");
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toBe("<script>alert(1)</script>");
  });

  it("turns both CRLF and LF line breaks into <br/>", () => {
    const crlf = renderMarkdown("a\r\nb");
    expect(crlf.querySelectorAll("br")).toHaveLength(1);
    expect(crlf.textContent).toBe("ab");

    const lf = renderMarkdown("a\nb\nc");
    expect(lf.querySelectorAll("br")).toHaveLength(2);
    expect(lf.textContent).toBe("abc");
  });
});
