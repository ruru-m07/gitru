import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { NavigationButtons } from "../src/components/custom-title-bar/navigation-buttons";

const createProps = () => ({
  activeTabId: "tab-1",
  canGoBack: true,
  canGoForward: false,
  isRootShellMode: false,
  goBack: vi.fn().mockResolvedValue({ current_path: "/app/git?view=history" }),
  goForward: vi.fn().mockResolvedValue({ current_path: "/app/git" }),
  navigate: vi.fn(),
});

beforeEach(() => {
  vi.spyOn(console, "log").mockImplementation(() => {});
});

describe("navigation button interactions", () => {
  test("runs an enabled navigation action and applies its returned path", async () => {
    const user = userEvent.setup();
    const props = createProps();

    render(<NavigationButtons {...props} />);

    const [backButton, forwardButton] = screen.getAllByRole("button");
    expect(backButton).toBeEnabled();
    expect(forwardButton).toBeDisabled();

    await user.click(backButton);

    expect(props.goBack).toHaveBeenCalledOnce();
    await waitFor(() => {
      expect(props.navigate).toHaveBeenCalledWith({
        to: "/app/git?view=history",
      });
    });
  });

  test("does not run disabled actions", async () => {
    const user = userEvent.setup();
    const props = createProps();

    render(
      <NavigationButtons {...props} canGoBack={false} canGoForward={false} />,
    );

    const [backButton, forwardButton] = screen.getAllByRole("button");
    await user.click(backButton);
    await user.click(forwardButton);

    expect(props.goBack).not.toHaveBeenCalled();
    expect(props.goForward).not.toHaveBeenCalled();
    expect(props.navigate).not.toHaveBeenCalled();
  });

  test("does not navigate when history has no destination", async () => {
    const user = userEvent.setup();
    const props = createProps();
    props.goBack.mockResolvedValue(null);

    render(<NavigationButtons {...props} />);

    const [backButton] = screen.getAllByRole("button");
    await user.click(backButton);

    expect(props.goBack).toHaveBeenCalledOnce();
    expect(props.navigate).not.toHaveBeenCalled();
  });
});
