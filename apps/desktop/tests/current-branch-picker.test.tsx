import type { BranchInfo } from "@gitru/commands";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
      side: _side,
      sideOffset: _sideOffset,
      tooltipStyle: _tooltipStyle,
      ...props
    }: ComponentProps<"div"> & {
      align?: string;
      alignOffset?: number;
      anchor?: unknown;
      side?: string;
      sideOffset?: number;
      tooltipStyle?: boolean;
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

vi.mock("@gitru/ui/components/menu", async () => {
  const React = await import("react");
  const TestMenuContext = React.createContext<{
    open: boolean;
    setOpen: (open: boolean) => void;
  } | null>(null);

  return {
    Menu: ({ children }: { children: ReactNode }) => {
      const [open, setOpen] = React.useState(false);
      return (
        <TestMenuContext.Provider value={{ open, setOpen }}>
          {children}
        </TestMenuContext.Provider>
      );
    },
    MenuTrigger: ({
      children,
      render,
    }: {
      children: ReactNode;
      render: ReactElement<{
        onClick?: MouseEventHandler<HTMLButtonElement>;
      }>;
    }) => {
      const context = React.useContext(TestMenuContext);
      if (!context) throw new Error("MenuTrigger must be inside Menu");

      return React.cloneElement(
        render,
        {
          onClick: (event) => {
            render.props.onClick?.(event);
            context.setOpen(!context.open);
          },
        },
        children,
      );
    },
    MenuPopup: ({
      align: _align,
      alignOffset: _alignOffset,
      anchor: _anchor,
      children,
      side: _side,
      sideOffset: _sideOffset,
      ...props
    }: ComponentProps<"div"> & {
      align?: string;
      alignOffset?: number;
      anchor?: unknown;
      side?: string;
      sideOffset?: number;
    }) => {
      const context = React.useContext(TestMenuContext);
      if (!context?.open) return null;
      return (
        <div {...props} role="menu">
          {children}
        </div>
      );
    },
    MenuItem: ({
      children,
      closeOnClick,
      onClick,
      variant: _variant,
      ...props
    }: ComponentProps<"button"> & {
      closeOnClick?: boolean;
      variant?: string;
    }) => {
      const context = React.useContext(TestMenuContext);
      return (
        <button
          {...props}
          type="button"
          role="menuitem"
          onClick={(event) => {
            onClick?.(event);
            if (closeOnClick) context?.setOpen(false);
          }}
        >
          {children}
        </button>
      );
    },
    MenuSeparator: (props: ComponentProps<"hr">) => (
      <hr {...props} role="separator" />
    ),
  };
});

vi.mock("@gitru/ui/components/scroll-area", () => ({
  ScrollArea: ({
    children,
    scrollFade: _scrollFade,
    scrollbarGutter: _scrollbarGutter,
    ...props
  }: ComponentProps<"div"> & {
    scrollFade?: boolean;
    scrollbarGutter?: boolean;
  }) => <div {...props}>{children}</div>,
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
    onFetch: vi.fn().mockResolvedValue(true),
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
  test("opens a dedicated branch popover with focused search", async () => {
    render(<CurrentBranchPicker {...createProps()} />);

    const popup = await openPicker();

    expect(popup).toBeInTheDocument();
    expect(
      screen.getByRole("searchbox", { name: "Filter branches" }),
    ).toHaveFocus();
    expect(
      screen.getByRole("list", { name: "Local branches" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Current branch main",
      }),
    ).toHaveAttribute("aria-current", "true");

    await closePicker();
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

  test("opening a branch action menu does not also check out the branch", async () => {
    const user = userEvent.setup();
    const props = createProps();
    render(<CurrentBranchPicker {...props} />);
    await openPicker();

    await user.click(
      screen.getByRole("button", {
        name: "Actions for branch feature/one",
      }),
    );

    expect(props.onSwitchBranch).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("menuitem", { name: "Rename branch" }),
    ).toBeInTheDocument();
    expect(props.onSwitchBranch).not.toHaveBeenCalled();

    fireEvent.keyDown(document, { key: "Escape" });
    await closePicker();
  });
});
