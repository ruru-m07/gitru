import type {
  Author,
  CommitInfo,
  GetStatusResponse,
  RepoOperation,
} from "@gitru/commands";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type {
  ComponentProps,
  MouseEventHandler,
  ReactElement,
  ReactNode,
} from "react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import {
  getCommitActionAvailability,
  WriteCommitBox,
} from "../src/features/git/components/write-commit-box";
import {
  joinCommitMessage,
  splitCommitMessage,
  useCommitDraftStore,
} from "../src/store/use-commit-draft-store";

const mocks = vi.hoisted(() => ({
  authors: [] as Author[],
  createCommit: vi.fn(),
  gitAdd: vi.fn(),
  lastCommit: null as CommitInfo | null,
  operation: null as RepoOperation | null,
  refetchLastCommit: vi.fn(),
  status: { files: [] } as GetStatusResponse,
  toastSuccess: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: mocks.toastSuccess,
  },
}));

vi.mock("@/state/use-active-repository-state", () => ({
  useActiveRepositoryState: () => ({ contextId: "repo-1" }),
}));

vi.mock("@/hooks", () => ({
  useCreateCommit: () => ({
    isPending: false,
    mutateAsync: mocks.createCommit,
  }),
  useGetCommitAuthors: () => ({
    data: mocks.authors,
    isLoading: false,
  }),
  useGetCurrentBranch: () => ({ data: { name: "main" } }),
  useGetLastCommit: () => ({
    data: mocks.lastCommit,
    isLoading: false,
    refetch: mocks.refetchLastCommit,
  }),
  useGetRepoOperation: () => ({
    data: mocks.operation,
    isLoading: false,
  }),
  useGetStatus: () => ({
    data: mocks.status,
    isLoading: false,
  }),
  useGitAdd: () => ({
    isPending: false,
    mutateAsync: mocks.gitAdd,
  }),
}));

vi.mock("@gitru/ui/components/menu", async () => {
  const React = await import("react");
  const MenuContext = React.createContext<{
    open: boolean;
    setOpen: (open: boolean) => void;
  } | null>(null);

  return {
    Menu: ({ children }: { children: ReactNode }) => {
      const [open, setOpen] = React.useState(false);
      return (
        <MenuContext.Provider value={{ open, setOpen }}>
          {children}
        </MenuContext.Provider>
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
      const context = React.useContext(MenuContext);
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
    MenuPopup: ({ children }: { children: ReactNode }) => {
      const context = React.useContext(MenuContext);
      return context?.open ? <div role="menu">{children}</div> : null;
    },
    MenuItem: ({
      children,
      closeOnClick: _closeOnClick,
      onClick,
      ...props
    }: ComponentProps<"button"> & { closeOnClick?: boolean }) => {
      const context = React.useContext(MenuContext);
      return (
        <button
          {...props}
          type="button"
          role="menuitem"
          onClick={(event) => {
            onClick?.(event);
            context?.setOpen(false);
          }}
        >
          {children}
        </button>
      );
    },
  };
});

vi.mock("@gitru/ui/components/alert-dialog", async () => {
  const React = await import("react");
  const AlertDialogContext = React.createContext<{
    open: boolean;
    onOpenChange: (open: boolean) => void;
  } | null>(null);

  return {
    AlertDialog: ({
      children,
      open,
      onOpenChange,
    }: {
      children: ReactNode;
      open: boolean;
      onOpenChange: (open: boolean) => void;
    }) => (
      <AlertDialogContext.Provider value={{ open, onOpenChange }}>
        {children}
      </AlertDialogContext.Provider>
    ),
    AlertDialogPopup: ({ children }: { children: ReactNode }) => {
      const context = React.useContext(AlertDialogContext);
      return context?.open ? <div role="alertdialog">{children}</div> : null;
    },
    AlertDialogHeader: (props: ComponentProps<"div">) => <div {...props} />,
    AlertDialogFooter: (props: ComponentProps<"div">) => <div {...props} />,
    AlertDialogTitle: (props: ComponentProps<"h2">) => <h2 {...props} />,
    AlertDialogDescription: (props: ComponentProps<"p">) => <p {...props} />,
    AlertDialogClose: ({
      children,
      render,
    }: {
      children: ReactNode;
      render: ReactElement<{
        onClick?: MouseEventHandler<HTMLButtonElement>;
      }>;
    }) => {
      const context = React.useContext(AlertDialogContext);
      return React.cloneElement(
        render,
        {
          onClick: (event) => {
            render.props.onClick?.(event);
            context?.onOpenChange(false);
          },
        },
        children,
      );
    },
  };
});

vi.mock("@gitru/ui/components/popover", async () => {
  const React = await import("react");
  const PopoverContext = React.createContext<{
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
      <PopoverContext.Provider value={{ open, onOpenChange }}>
        {children}
      </PopoverContext.Provider>
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
      const context = React.useContext(PopoverContext);
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
      children,
      align: _align,
      side: _side,
      viewport: _viewport,
      ...props
    }: ComponentProps<"div"> & {
      align?: string;
      side?: string;
      viewport?: boolean;
    }) => {
      const context = React.useContext(PopoverContext);
      return context?.open ? <div {...props}>{children}</div> : null;
    },
    PopoverTitle: (props: ComponentProps<"h2">) => <h2 {...props} />,
    PopoverDescription: (props: ComponentProps<"p">) => <p {...props} />,
  };
});

vi.mock("@gitru/ui/components/combobox", async () => {
  const React = await import("react");
  const ComboboxContext = React.createContext<{
    items: Author[];
    query: string;
    setQuery: (query: string) => void;
    value: Author[];
    onValueChange: (authors: Author[]) => void;
  } | null>(null);

  return {
    Combobox: ({
      children,
      items,
      value,
      onValueChange,
    }: {
      children: ReactNode;
      items: Author[];
      value: Author[];
      onValueChange: (authors: Author[]) => void;
    }) => {
      const [query, setQuery] = React.useState("");
      const normalizedQuery = query.trim().toLocaleLowerCase();
      const filteredItems = items.filter((item) =>
        `${item.name} ${item.email}`
          .toLocaleLowerCase()
          .includes(normalizedQuery),
      );
      return (
        <ComboboxContext.Provider
          value={{
            items: filteredItems,
            query,
            setQuery,
            value,
            onValueChange,
          }}
        >
          {children}
        </ComboboxContext.Provider>
      );
    },
    ComboboxChips: (props: ComponentProps<"div">) => <div {...props} />,
    ComboboxInput: ({
      showTrigger: _showTrigger,
      onChange,
      ...props
    }: ComponentProps<"input"> & { showTrigger?: boolean }) => {
      const context = React.useContext(ComboboxContext);
      return (
        <input
          type="search"
          {...props}
          value={context?.query ?? ""}
          onChange={(event) => {
            onChange?.(event);
            context?.setQuery(event.target.value);
          }}
        />
      );
    },
    ComboboxEmpty: ({ children }: { children: ReactNode }) => {
      const context = React.useContext(ComboboxContext);
      return context?.items.length === 0 ? <div>{children}</div> : null;
    },
    ComboboxList: ({
      children,
    }: {
      children: (author: Author) => ReactNode;
    }) => {
      const context = React.useContext(ComboboxContext);
      return <div role="listbox">{context?.items.map(children)}</div>;
    },
    ComboboxItem: ({
      children,
      value,
    }: {
      children: ReactNode;
      value: Author;
    }) => {
      const context = React.useContext(ComboboxContext);
      const selected =
        context?.value.some(
          (author) =>
            author.name === value.name && author.email === value.email,
        ) ?? false;
      return (
        <button
          type="button"
          aria-selected={selected}
          role="option"
          onClick={() => {
            if (!context) return;
            context.onValueChange(
              selected
                ? context.value.filter(
                    (author) =>
                      author.name !== value.name ||
                      author.email !== value.email,
                  )
                : [...context.value, value],
            );
          }}
        >
          {children}
        </button>
      );
    },
  };
});

function operation(overrides: Partial<RepoOperation> = {}): RepoOperation {
  return {
    kind: "clean",
    isRebasing: false,
    todo: [],
    conflictPaths: [],
    ...overrides,
  };
}

function author(name: string, email: string): Author {
  return { name, email };
}

function commit(overrides: Partial<CommitInfo> = {}): CommitInfo {
  const currentAuthor = author("Ruru", "ruru@example.com");
  return {
    id: "0123456789abcdef",
    summary: "Existing summary",
    body: "Existing body",
    timestamp: 1,
    authors: {
      author: currentAuthor,
      committer: currentAuthor,
      co_authors: [],
    },
    ...overrides,
  };
}

function setDraft({
  title,
  description = "",
  coAuthors = [],
}: {
  title: string;
  description?: string;
  coAuthors?: Array<[string, string]>;
}) {
  useCommitDraftStore.setState({
    repoKey: "repo-1",
    title,
    description,
    coAuthors,
    mode: "create",
    amendCommitId: null,
    draftBeforeAmend: null,
    autofillKey: null,
  });
}

async function openCommitOptions(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Commit options" }));
  return screen.getByRole("menu");
}

beforeEach(() => {
  mocks.authors = [author("Alice", "alice@example.com")];
  mocks.createCommit.mockResolvedValue("new-commit-id");
  mocks.gitAdd.mockResolvedValue(undefined);
  mocks.lastCommit = commit();
  mocks.refetchLastCommit.mockImplementation(async () => ({
    data: mocks.lastCommit,
    error: null,
  }));
  mocks.operation = operation();
  mocks.status = { files: [] };
  setDraft({ title: "" });
});

describe("WriteCommitBox", () => {
  test("uses a deliberate confirmation for empty commits and never stages visible files", async () => {
    const user = userEvent.setup();
    setDraft({ title: "Checkpoint" });
    render(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    let menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Empty Commit…" }),
    );

    let dialog = screen.getByRole("alertdialog");
    expect(
      within(dialog).getByRole("heading", { name: "Create an empty commit?" }),
    ).toBeInTheDocument();
    expect(mocks.createCommit).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();

    menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Empty Commit…" }),
    );
    dialog = screen.getByRole("alertdialog");
    await user.click(
      within(dialog).getByRole("button", { name: "Create empty commit" }),
    );

    await waitFor(() => {
      expect(mocks.createCommit).toHaveBeenCalledWith({
        commitMeta: {
          title: "Checkpoint",
          description: "",
          co_authors: [],
        },
        allowEmpty: true,
        amend: false,
        expectedHead: undefined,
      });
    });
    expect(mocks.gitAdd).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  test("rechecks empty-commit eligibility while confirmation is open", async () => {
    const user = userEvent.setup();
    setDraft({ title: "Checkpoint" });
    const { rerender } = render(
      <WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />,
    );

    const menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Empty Commit…" }),
    );

    mocks.status = {
      files: [{ path: "newly-staged.txt", status: ["IndexNew"] }],
    };
    rerender(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    const confirm = within(screen.getByRole("alertdialog")).getByRole(
      "button",
      { name: "Create empty commit" },
    );
    expect(confirm).toBeDisabled();
    await user.click(confirm);
    expect(mocks.createCommit).not.toHaveBeenCalled();
    expect(mocks.gitAdd).not.toHaveBeenCalled();
  });

  test("sends no expected HEAD for a regular commit", async () => {
    const user = userEvent.setup();
    mocks.status = {
      files: [{ path: "staged.txt", status: ["IndexModified"] }],
    };
    setDraft({ title: "Regular commit", description: "Body" });
    render(<WriteCommitBox visibleAddablePaths={[]} />);

    await user.click(screen.getByRole("button", { name: "Commit to main" }));

    await waitFor(() => {
      expect(mocks.createCommit).toHaveBeenCalledWith({
        commitMeta: {
          title: "Regular commit",
          description: "Body",
          co_authors: [],
        },
        allowEmpty: false,
        amend: false,
        expectedHead: undefined,
      });
    });
  });

  test.each([
    { staged: false, modeLabel: "Amending 0123456 (message only)" },
    { staged: true, modeLabel: "Amending 0123456 with staged changes" },
  ])("prefills and submits amend mode without auto-staging (staged: $staged)", async ({
    staged,
    modeLabel,
  }) => {
    const user = userEvent.setup();
    const existingCoAuthor = author("Existing Pair", "pair@example.com");
    mocks.lastCommit = commit({
      body: [
        "Existing body",
        "# Kept body line",
        "",
        "Co-authored-by: Existing Pair <pair@example.com>",
        "Signed-off-by: Ruru <ruru@example.com>",
      ].join("\n"),
      authors: {
        author: author("Ruru", "ruru@example.com"),
        committer: author("Ruru", "ruru@example.com"),
        co_authors: [existingCoAuthor],
      },
    });
    mocks.status = staged
      ? {
          files: [{ path: "staged.txt", status: ["IndexModified"] }],
        }
      : { files: [] };
    setDraft({ title: "Draft summary", description: "Draft body" });
    render(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    const menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Amend Last Commit" }),
    );

    expect(screen.getByRole("textbox", { name: "Commit summary" })).toHaveValue(
      "Existing summary",
    );
    expect(
      screen.getByRole("textbox", { name: "Commit description" }),
    ).toHaveValue(
      "Existing body\n# Kept body line\n\nSigned-off-by: Ruru <ruru@example.com>",
    );
    expect(screen.getByRole("status")).toHaveTextContent(modeLabel);
    expect(
      screen.getByRole("button", { name: "Remove co-author Existing Pair" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Amend last commit" }));

    await waitFor(() => {
      expect(mocks.createCommit).toHaveBeenCalledWith({
        commitMeta: {
          title: "Existing summary",
          description:
            "Existing body\n# Kept body line\n\nSigned-off-by: Ruru <ruru@example.com>",
          co_authors: [["Existing Pair", "pair@example.com"]],
        },
        allowEmpty: false,
        amend: true,
        expectedHead: "0123456789abcdef",
      });
    });
    expect(mocks.gitAdd).not.toHaveBeenCalled();
  });

  test("canceling amend restores the draft that was being written", async () => {
    const user = userEvent.setup();
    setDraft({
      title: "Draft summary",
      description: "Draft body",
      coAuthors: [["Draft Pair", "draft@example.com"]],
    });
    render(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    const menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Amend Last Commit" }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel amend" }));

    expect(screen.getByRole("textbox", { name: "Commit summary" })).toHaveValue(
      "Draft summary",
    );
    expect(
      screen.getByRole("textbox", { name: "Commit description" }),
    ).toHaveValue("Draft body");
    expect(
      screen.getByRole("button", { name: "Remove co-author Draft Pair" }),
    ).toBeInTheDocument();
  });

  test("refetches the paused commit before entering rebase amend mode", async () => {
    const user = userEvent.setup();
    mocks.lastCommit = commit({
      id: "old-tip",
      summary: "Before rebase",
    });
    const freshCommit = commit({
      id: "fresh-replayed-tip",
      summary: "Paused commit",
      body: "Paused body",
      authors: {
        author: author("Ruru", "ruru@example.com"),
        committer: author("Ruru", "ruru@example.com"),
        co_authors: [author("Fresh Pair", "fresh@example.com")],
      },
    });
    mocks.refetchLastCommit.mockResolvedValue({
      data: freshCommit,
      error: null,
    });
    mocks.operation = operation({
      kind: "rebaseInteractive",
      isRebasing: true,
      pauseReason: "edit",
      pausedAt: "original-paused-commit",
      commitMessage: "Paused commit\n\nPaused body",
    });
    setDraft({ title: "Before operation refresh" });
    render(<WriteCommitBox visibleAddablePaths={[]} />);

    const menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Amend Last Commit" }),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("textbox", { name: "Commit summary" }),
      ).toHaveValue("Paused commit");
    });
    expect(mocks.refetchLastCommit).toHaveBeenCalledOnce();
    expect(
      screen.getByRole("button", { name: "Remove co-author Fresh Pair" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent(
      "Amending fresh-r (message only)",
    );
    expect(useCommitDraftStore.getState().amendCommitId).toBe(
      "fresh-replayed-tip",
    );
  });

  test("a rejected expected-HEAD amend preserves the full draft and amend mode", async () => {
    const user = userEvent.setup();
    const existingCoAuthor = author("Existing Pair", "pair@example.com");
    mocks.lastCommit = commit({
      authors: {
        author: author("Ruru", "ruru@example.com"),
        committer: author("Ruru", "ruru@example.com"),
        co_authors: [existingCoAuthor],
      },
    });
    mocks.createCommit.mockRejectedValue(
      new Error("HEAD changed since amend mode was opened"),
    );
    setDraft({ title: "Draft summary" });
    render(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    const menu = await openCommitOptions(user);
    await user.click(
      within(menu).getByRole("menuitem", { name: "Amend Last Commit" }),
    );
    const summary = screen.getByRole("textbox", { name: "Commit summary" });
    const description = screen.getByRole("textbox", {
      name: "Commit description",
    });
    await user.clear(summary);
    await user.type(summary, "Retried summary");
    await user.clear(description);
    await user.type(description, "Retried body");
    await user.click(screen.getByRole("button", { name: "Amend last commit" }));

    await waitFor(() => expect(mocks.createCommit).toHaveBeenCalledOnce());
    expect(mocks.createCommit).toHaveBeenCalledWith({
      commitMeta: {
        title: "Retried summary",
        description: "Retried body",
        co_authors: [["Existing Pair", "pair@example.com"]],
      },
      allowEmpty: false,
      amend: true,
      expectedHead: "0123456789abcdef",
    });
    expect(summary).toHaveValue("Retried summary");
    expect(description).toHaveValue("Retried body");
    expect(
      screen.getByRole("button", { name: "Cancel amend" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Remove co-author Existing Pair" }),
    ).toBeInTheDocument();
    expect(mocks.gitAdd).not.toHaveBeenCalled();
  });

  test("fails closed and preserves the draft when amend mode has no expected HEAD", async () => {
    const user = userEvent.setup();
    setDraft({
      title: "Unprotected amend",
      description: "Must stay",
      coAuthors: [["Pair", "pair@example.com"]],
    });
    useCommitDraftStore.setState({
      mode: "amend",
      amendCommitId: null,
      draftBeforeAmend: null,
    });
    render(<WriteCommitBox visibleAddablePaths={[]} />);

    const submit = screen.getByRole("button", { name: "Amend last commit" });
    expect(submit).toBeDisabled();
    await user.click(submit);

    expect(mocks.createCommit).not.toHaveBeenCalled();
    expect(screen.getByRole("textbox", { name: "Commit summary" })).toHaveValue(
      "Unprotected amend",
    );
    expect(
      screen.getByRole("textbox", { name: "Commit description" }),
    ).toHaveValue("Must stay");
    expect(
      screen.getByRole("button", { name: "Remove co-author Pair" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel amend" })).toBeVisible();
  });

  test("selects co-authors from a searchable picker and exposes the selection", async () => {
    const user = userEvent.setup();
    mocks.authors = [
      author("Alice", "alice@example.com"),
      author("Bob", "bob@example.com"),
    ];
    setDraft({ title: "Pair work" });
    render(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    await user.click(screen.getByRole("button", { name: "Add co-authors" }));
    const search = screen.getByRole("searchbox", { name: "Search co-authors" });
    await user.type(search, "bob@example.com");
    expect(screen.queryByRole("option", { name: /Alice/ })).toBeNull();
    await user.click(screen.getByRole("option", { name: /Bob/ }));

    await user.clear(search);
    await user.type(search, "Alice");
    await user.click(screen.getByRole("option", { name: /Alice/ }));

    expect(
      screen.getByRole("button", { name: "Remove co-author Alice" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Remove co-author Bob" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Manage co-authors, 2 selected" }),
    ).toBeInTheDocument();
  });

  test("deduplicates same-email authors into one option and one trailer", async () => {
    const user = userEvent.setup();
    mocks.authors = [
      author("Alice", "pair@example.com"),
      author("Alicia", "PAIR@example.com"),
    ];
    mocks.status = {
      files: [{ path: "staged.txt", status: ["IndexModified"] }],
    };
    setDraft({ title: "Pair work" });
    render(<WriteCommitBox visibleAddablePaths={[]} />);

    await user.click(screen.getByRole("button", { name: "Add co-authors" }));
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(1);
    expect(options[0]).toHaveTextContent("Alice");
    await user.click(options[0]);
    await user.click(screen.getByRole("button", { name: "Commit to main" }));

    await waitFor(() => {
      expect(mocks.createCommit).toHaveBeenCalledWith({
        commitMeta: {
          title: "Pair work",
          description: "",
          co_authors: [["Alice", "pair@example.com"]],
        },
        allowEmpty: false,
        amend: false,
        expectedHead: undefined,
      });
    });
  });

  test("requires a non-whitespace summary and removes the inert Sparkles action", () => {
    setDraft({ title: "   " });
    render(<WriteCommitBox visibleAddablePaths={["unstaged.txt"]} />);

    expect(
      screen.getByRole("button", { name: "Add visible & Commit" }),
    ).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: "Password requirements" }),
    ).not.toBeInTheDocument();
  });
});

describe("getCommitActionAvailability", () => {
  test("blocks every commit mode when a clean operation reports conflicts", () => {
    expect(
      getCommitActionAvailability(
        operation({ kind: "clean", conflictPaths: ["stash-conflict.txt"] }),
      ),
    ).toEqual({
      create: false,
      empty: false,
      amend: false,
      isRebaseLike: false,
    });
  });

  test.each([
    "merge",
    "revert",
    "cherryPick",
  ] as const)("allows regular commits after a resolved %s", (kind) => {
    expect(getCommitActionAvailability(operation({ kind }))).toEqual({
      create: true,
      empty: false,
      amend: false,
      isRebaseLike: false,
    });
  });

  test.each([
    "merge",
    "revert",
    "cherryPick",
  ] as const)("blocks regular commits while a %s still has conflicts", (kind) => {
    expect(
      getCommitActionAvailability(
        operation({ kind, conflictPaths: ["conflicted.txt"] }),
      ).create,
    ).toBe(false);
  });

  test("blocks ordinary commits during apply-mailbox and unknown operations", () => {
    expect(
      getCommitActionAvailability(operation({ kind: "applyMailbox" })).create,
    ).toBe(false);
    expect(
      getCommitActionAvailability(operation({ kind: "other" })).create,
    ).toBe(false);
  });

  test("allows only amend at a conflict-free rebase edit pause", () => {
    expect(
      getCommitActionAvailability(
        operation({
          kind: "rebaseInteractive",
          isRebasing: true,
          pauseReason: "edit",
        }),
      ),
    ).toEqual({
      create: false,
      empty: false,
      amend: true,
      isRebaseLike: true,
    });
  });

  test("blocks amend at incompatible rebase pauses or while conflicts remain", () => {
    expect(
      getCommitActionAvailability(
        operation({
          kind: "rebase",
          isRebasing: true,
          pauseReason: "reword",
        }),
      ).amend,
    ).toBe(false);
    expect(
      getCommitActionAvailability(
        operation({
          kind: "rebase",
          isRebasing: true,
          pauseReason: "edit",
          conflictPaths: ["conflicted.txt"],
        }),
      ).amend,
    ).toBe(false);
  });
});

test("rebuilds commit messages with co-author trailers", () => {
  expect(
    joinCommitMessage("Pair change", "Explains the change", [
      ["Alice", "alice@example.com"],
      ["Bob", "bob@example.com"],
    ]),
  ).toBe(
    "Pair change\n\nExplains the change\n\nCo-authored-by: Alice <alice@example.com>\nCo-authored-by: Bob <bob@example.com>",
  );
  expect(
    joinCommitMessage("fix: preserve trailer spacing", "", [
      ["Alice", "alice@example.com"],
    ]),
  ).toBe(
    "fix: preserve trailer spacing\n\nCo-authored-by: Alice <alice@example.com>",
  );
});

test("extracts co-authors from a mixed trailing trailer block", () => {
  const parsed = splitCommitMessage(
    "Pair change\n\n# Details\nBody\n\nCo-authored-by: Alice <alice@example.com>\nSigned-off-by: Ruru <ruru@example.com>\nChange-Id: I123456",
  );
  expect(parsed).toEqual({
    title: "Pair change",
    description:
      "# Details\nBody\n\nSigned-off-by: Ruru <ruru@example.com>\nChange-Id: I123456",
    coAuthors: [["Alice", "alice@example.com"]],
  });

  const rebuilt = joinCommitMessage(
    parsed.title,
    parsed.description,
    parsed.coAuthors,
  );
  expect(rebuilt).toBe(
    "Pair change\n\n# Details\nBody\n\nSigned-off-by: Ruru <ruru@example.com>\nChange-Id: I123456\nCo-authored-by: Alice <alice@example.com>",
  );
  expect(splitCommitMessage(rebuilt)).toEqual(parsed);
});
