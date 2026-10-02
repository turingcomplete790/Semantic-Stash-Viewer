import { cleanup, fireEvent, render } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import PageControls from "../../scenes/PageControls";

function setup(page: number, pageSize = 50, count = 36_350) {
  const onPage = vi.fn();
  const onPageSize = vi.fn();
  const r = render(() => (
    <PageControls
      page={page}
      pageSize={pageSize}
      count={count}
      onPage={onPage}
      onPageSize={onPageSize}
    />
  ));
  return { ...r, onPage, onPageSize };
}

afterEach(cleanup);

describe("PageControls", () => {
  it("says where the page is in the library", () => {
    const c = setup(25);
    expect(c.getByTestId("page-position")).toHaveTextContent(
      "Page 25 of 727 · 1,201–1,250 of 36,350",
    );
    cleanup();
    const last = setup(727);
    expect(last.getByTestId("page-position")).toHaveTextContent(
      "Page 727 of 727 · 36,301–36,350 of 36,350",
    );
  });

  it("disables the buttons that lead nowhere", () => {
    const first = setup(1);
    expect(first.getByRole("button", { name: "First page" })).toBeDisabled();
    expect(first.getByRole("button", { name: "Previous page" })).toBeDisabled();
    expect(first.getByRole("button", { name: "Next page" })).toBeEnabled();
    cleanup();
    const last = setup(727);
    expect(last.getByRole("button", { name: "Next page" })).toBeDisabled();
    expect(last.getByRole("button", { name: "Last page" })).toBeDisabled();
  });

  it("moves between pages", () => {
    const c = setup(25);
    fireEvent.click(c.getByRole("button", { name: "Next page" }));
    expect(c.onPage).toHaveBeenLastCalledWith(26);
    fireEvent.click(c.getByRole("button", { name: "Previous page" }));
    expect(c.onPage).toHaveBeenLastCalledWith(24);
    fireEvent.click(c.getByRole("button", { name: "First page" }));
    expect(c.onPage).toHaveBeenLastCalledWith(1);
    fireEvent.click(c.getByRole("button", { name: "Last page" }));
    expect(c.onPage).toHaveBeenLastCalledWith(727);
  });

  it("jumps to a page, clamped to the last one, and ignores nonsense", () => {
    const c = setup(1);
    const input = c.getByLabelText("Go to page");
    fireEvent.input(input, { target: { value: "9999" } });
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    expect(c.onPage).toHaveBeenLastCalledWith(727);
    c.onPage.mockClear();
    fireEvent.input(input, { target: { value: "0" } });
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    fireEvent.input(input, { target: { value: "abc" } });
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    expect(c.onPage).not.toHaveBeenCalled();
  });

  it("offers Stash's page sizes plus 50", () => {
    const c = setup(1);
    const select = c.getByLabelText("Per page") as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.value)).toEqual([
      "20",
      "40",
      "50",
      "60",
      "120",
      "250",
      "500",
      "1000",
    ]);
    expect(select.value).toBe("50");
    fireEvent.change(select, { target: { value: "120" } });
    expect(c.onPageSize).toHaveBeenLastCalledWith(120);
  });
});
