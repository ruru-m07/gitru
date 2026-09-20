import type { BranchInfo } from "@gitru/commands";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type {
  ComponentProps,
  MouseEventHandler,
  ReactElement,
  ReactNode,
} from "react";
import { describe, expect, test, vi } from "vitest";
import {
  CurrentBranchPicker,
  type CurrentBranchPickerProps,
} from "../src/features/git/components/current-branch-control";

vi.mock("@gitru/ui/components/popover", async () => {
  const React = await import("react");
  const TestPopoverContext = React.createContext<{
    open: boolean;
    onOpenChange: (open: boolean) => void;
  } | null>(null);

  return {
    Popover: ({
      children,
      open,
      onOpenChange,
    }: {
      children: ReactNode;
      open: boolean;
      onOpenChange: (open: boolean) => void;
    }) => (
      <TestPopoverContext.Provider value={{ open, onOpenChange }}>
        {children}
      </TestPopoverContext.Provider>
    ),
    PopoverTrigger: ({
      children,
      render,
    }: {
      children: ReactNode;
      render: ReactElement<{
        onClick?: MouseEventHandler<HTMLButtonElement>;
      }>;
    }) => {
      const context = React.useContext(TestPopoverContext);
      if (!context) throw new Error("PopoverTrigger must be inside Popover");

      return React.cloneElement(
        render,
        {
          onClick: (event) => {
            render.props.onClick?.(event);
            context.onOpenChange(!context.open);
          },
        },
        children,
      );
    },
    PopoverPopup: ({
      align: _align,
      alignOffset: _alignOffset,
      anchor: _anchor,
      children,
      collisionAvoidance: _collisionAvoidance,
      collisionPadding: _collisionPadding,
      positionMethod: _positionMethod,
      side: _side,
      sideOffset: _sideOffset,
      tooltipStyle: _tooltipStyle,
      viewport: _viewport,
      viewportClassName: _viewportClassName,
      ...props
    }: ComponentProps<"div"> & {
      align?: string;
      alignOffset?: number;
      anchor?: unknown;
      collisionAvoidance?: unknown;
      collisionPadding?: number | object;
      positionMethod?: string;
      side?: string;
      sideOffset?: number;
      tooltipStyle?: boolean;
      viewport?: boolean;
      viewportClassName?: string;
    }) => {
      const context = React.useContext(TestPopoverContext);
      if (!context?.open) return null;

      return (
        <div {...props} aria-label="Branches" role="dialog">
          {children}
        </div>
      );
    },
    PopoverTitle: (props: ComponentProps<"h2">) => <h2 {...props} />,
    PopoverDescription: (props: ComponentProps<"p">) => <p {...props} />,
  };
});

vi.mock("@gitru/ui/components/context-menu", async () => {
  const React = await import("react");
  const TestContextMenuContext = React.createContext<{
    open: boolean;
    setOpen: (open: boolean) => void;
  } | null>(null);

  return {
    ContextMenu: ({ children }: { children: ReactNode }) => {
      const [open, setOpen] = React.useState(false);
      return (
        <TestContextMenuContext.Provider value={{ open, setOpen }}>
          {children}
        </TestContextMenuContext.Provider>
      );
    },
    ContextMenuTrigger: ({
      asChild: _asChild,
      children,
    }: {
      asChild?: boolean;
      children: ReactElement<{
        onContextMenu?: MouseEventHandler<HTMLButtonElement>;
      }>;
    }) => {
      const context = React.useContext(TestContextMenuContext);
      if (!context) {
        throw new Error("ContextMenuTrigger must be inside ContextMenu");
      }

      return React.cloneElement(children, {
        onContextMenu: (event) => {
          children.props.onContextMenu?.(event);
          event.preventDefault();
          context.setOpen(true);
        },
      });
    },
    ContextMenuContent: ({
      children,
      onEscapeKeyDown: _onEscapeKeyDown,
      ...props
    }: ComponentProps<"div"> & {
      onEscapeKeyDown?: (event: KeyboardEvent) => void;
    }) => {
      const context = React.useContext(TestContextMenuContext);
      if (!context?.open) return null;
      return (
        <div {...props} role="menu">
          {children}
        </div>
      );
    },
    ContextMenuItem: ({
      children,
      onSelect,
      ...props
    }: Omit<ComponentProps<"button">, "onSelect"> & {
      onSelect?: (event: Event) => void;
    }) => {
      const context = React.useContext(TestContextMenuContext);
      return (
        <button
          {...props}
          type="button"
          role="menuitem"
          onClick={(event) => {
            onSelect?.(event.nativeEvent);
            context?.setOpen(false);
          }}
        >
          {children}
        </button>
      );
    },
    ContextMenuSeparator: (props: ComponentProps<"hr">) => (
      <hr {...props} role="separator" />
    ),
  };
});

vi.mock("@gitru/ui/components/scroll-area", () => ({
  ScrollArea: ({
    children,
    scrollFade: _scrollFade,
    scrollbarGutter: _scrollbarGutter,
    viewportRef,
    ...props
  }: ComponentProps<"div"> & {
    scrollFade?: boolean;
    scrollbarGutter?: boolean;
    viewportRef?: ComponentProps<"div">["ref"];
  }) => (
    <div ref={viewportRef} {...props}>
      {children}
    </div>
  ),
}));

const author = {
  name: "Ruru",
  email: "ruru@example.com",
};

function branch(name: string, overrides: Partial<BranchInfo> = {}): BranchInfo {
  return {
    name,
    display_name: name,
    is_remote: false,
    is_head: false,
    commit: {
      id: `${name}-commit`,
      summary: `Commit on ${name}`,
      body: "",
      timestamp: 1,
      authors: {
        author,
        committer: author,
        co_authors: [],
      },
    },
    is_protected: false,
    is_merged: false,
    ...overrides,
  };
}

function createProps(
  overrides: Partial<CurrentBranchPickerProps> = {},
): CurrentBranchPickerProps {
  return {
    currentBranchName: "main",
    currentBranchDisplayName: "main",
    detached: false,
    rebasing: false,
    localBranches: [
      branch("main", { is_head: true, is_merged: true }),
      branch("feature/one"),
      branch("feature/日本語"),
      branch("fix/other"),
    ],
    remoteBranches: [
      branch("upstream/feature/日本語", {
        display_name: "upstream/feature/日本語",
        is_remote: true,
      }),
    ],
    hasUncommittedChanges: false,
    onSwitchBranch: vi.fn().mockResolvedValue(true),
    onCreateBranch: vi.fn().mockResolvedValue(true),
    onRenameBranch: vi.fn().mockResolvedValue(true),
    onDeleteBranch: vi.fn().mockResolvedValue(true),
    onSetUpstream: vi.fn().mockResolvedValue(true),
    onUnsetUpstream: vi.fn().mockResolvedValue(true),
    ...overrides,
  };
}

async function openPicker() {
  const trigger = document.querySelector<HTMLButtonElement>(
    '[aria-label="Current branch: main. Open branches."]',
  );
  if (!trigger) throw new Error("Current branch trigger was not rendered");
  fireEvent.click(trigger);

  return await screen.findByRole("dialog", { name: "Branches" });
}

async function closePicker() {
  const trigger = document.querySelector<HTMLButtonElement>(
    '[aria-label="Current branch: main. Open branches."]',
  );
  if (!trigger) throw new Error("Current branch trigger was not rendered");
  fireEvent.click(trigger);
  await waitFor(() => {
    expect(
      screen.queryByRole("searchbox", { name: "Filter branches" }),
    ).not.toBeInTheDocument();
  });
}

describe("CurrentBranchPicker", () => {
  test("opens a dedicated branch panel with focused search", async () => {
    const user = userEvent.setup();
    const props = createProps();
    render(<CurrentBranchPicker {...props} />);

    const popup = await openPicker();

    expect(popup).toBeInTheDocument();
    expect(popup).toHaveAttribute("data-current-branch-panel");
    expect(popup).toHaveStyle({
      height:
        "calc(var(--available-height) - var(--main-actual-content-padding) - var(--main-status-bar-height))",
    });
    expect(
      screen.getByRole("searchbox", { name: "Filter branches" }),
    ).toHaveFocus();
    expect(screen.getByRole("tab", { name: /Local/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Remote/ })).toBeInTheDocument();
    expect(
      screen.getByRole("list", { name: "Local branches" }),
    ).toBeInTheDocument();
    expect(
      within(popup).getByRole("heading", { name: "Current Branch" }),
    ).toBeInTheDocument();
    expect(
      within(popup).getByRole("heading", { name: "Other Branches" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Fetch & prune|Fetching/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Current branch main",
      }),
    ).toHaveAttribute("aria-current", "true");

    await user.click(
      screen.getByRole("button", {
        name: "Current branch main",
      }),
    );
    expect(props.onSwitchBranch).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(
        screen.queryByRole("searchbox", { name: "Filter branches" }),
      ).not.toBeInTheDocument();
    });
  });

  test("filters branch names containing slashes and Unicode", async () => {
    const user = userEvent.setup();
    render(<CurrentBranchPicker {...createProps()} />);
    await openPicker();

    await user.type(
      screen.getByRole("searchbox", { name: "Filter branches" }),
      "feature/日本",
    );

    expect(
      screen.getByRole("button", { name: "Checkout feature/日本語" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Checkout feature/one" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Checkout fix/other" }),
    ).not.toBeInTheDocument();

    await closePicker();
  });

  test("shows a compact branch age and exposes its exact time", async () => {
    const timestamp =
      Math.floor(Date.now() / 1000) - 12 * 7 * 24 * 60 * 60 - 60 * 60;
    const currentBranch = branch("main", { is_head: true, is_merged: true });
    currentBranch.commit = { ...currentBranch.commit, timestamp };
    render(
      <CurrentBranchPicker
        {...createProps({ localBranches: [currentBranch] })}
      />,
    );
    await openPicker();

    const row = screen.getByRole("button", { name: "Current branch main" });
    const compactTime = within(row).getByText("12w");
    const exactTime = new Date(timestamp * 1000).toLocaleString();

    expect(compactTime).toHaveAttribute(
      "datetime",
      new Date(timestamp * 1000).toISOString(),
    );
    expect(row).toHaveAttribute(
      "aria-description",
      `Last commit ${exactTime}. Right-click for branch actions.`,
    );
  });

  test("checks out a clean local branch and closes the popover", async () => {
    const user = userEvent.setup();
    const props = createProps();
    render(<CurrentBranchPicker {...props} />);
    await openPicker();

    await user.click(
      screen.getByRole("button", { name: "Checkout feature/one" }),
    );

    expect(props.onSwitchBranch).toHaveBeenCalledOnce();
    expect(props.onSwitchBranch).toHaveBeenCalledWith("feature/one");
    await waitFor(() => {
      expect(
        screen.queryByRole("searchbox", { name: "Filter branches" }),
      ).not.toBeInTheDocument();
    });
  });

  test("passes the full remote branch name when checking out and tracking", async () => {
    const user = userEvent.setup();
    const props = createProps();
    render(<CurrentBranchPicker {...props} />);
    await openPicker();

    await user.click(screen.getByRole("tab", { name: /Remote/ }));
    await user.click(
      screen.getByRole("button", {
        name: "Checkout and track upstream/feature/日本語",
      }),
    );

    expect(props.onSwitchBranch).toHaveBeenCalledOnce();
    expect(props.onSwitchBranch).toHaveBeenCalledWith(
      "upstream/feature/日本語",
    );
  });

  test("virtualizes thousands of branches and still filters to a deep match", async () => {
    const user = userEvent.setup();
    const remoteBranches = Array.from({ length: 2_000 }, (_, index) =>
      branch(`origin/fixture/remote-${String(index + 1).padStart(4, "0")}`, {
        is_remote: true,
      }),
    );
    render(
      <CurrentBranchPicker
        {...createProps({
          remoteBranches,
        })}
      />,
    );
    await openPicker();

    await user.click(screen.getByRole("tab", { name: /Remote/ }));

    const list = screen.getByRole("list", { name: "Remote branches" });
    expect(list).toHaveAttribute("data-virtualized", "true");
    expect(Number.parseFloat(list.style.height)).toBeGreaterThan(70_000);
    expect(list.querySelectorAll("[data-branch-row]").length).toBeLessThan(100);

    await user.type(
      screen.getByRole("searchbox", { name: "Filter branches" }),
      "remote-2000",
    );

    expect(list).toHaveAttribute("data-virtualized", "false");
    expect(
      screen.getByRole("button", {
        name: "Checkout and track origin/fixture/remote-2000",
      }),
    ).toBeInTheDocument();
  });

  test("right-click opens branch actions without reserving an action button", async () => {
    const props = createProps();
    render(<CurrentBranchPicker {...props} />);
    await openPicker();

    expect(
      screen.queryByRole("button", {
        name: "Actions for branch feature/one",
      }),
    ).not.toBeInTheDocument();

    fireEvent.contextMenu(
      screen.getByRole("button", { name: "Checkout feature/one" }),
    );

    expect(props.onSwitchBranch).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("menuitem", { name: "Rename branch" }),
    ).toBeInTheDocument();
    expect(props.onSwitchBranch).not.toHaveBeenCalled();

    await closePicker();
  });
});
