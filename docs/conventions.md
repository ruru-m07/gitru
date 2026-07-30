# Gitru Code Conventions

## File and folder naming

| Kind | Convention | Example |
|------|------------|---------|
| Source files | kebab-case | `repository-list-item.tsx` |
| Hooks | `use-kebab-case.ts` | `use-repository.ts` |
| Folders | kebab-case | `action-panel/` |
| React exports | PascalCase inside files | `export function RepositoryListItem()` |
| TanStack routes | keep framework names | `route.tsx`, `index.tsx`, `__root.tsx` |
| Generated files | unchanged | `routeTree.gen.ts` |
| Web workers | kebab-case.worker.ts | `pixel-diff.worker.ts` |

**Exceptions:** Simple lowercase single-word files are allowed when they are not compound names (`logo.tsx`, `colors.ts`, `time.ts`).

## State boundaries

- **Zustand stores** (`store/*`) — UI/workspace persistence: tabs, panel layout, file selection, theme preferences.
- **Domain state** (`state/domains/*`) — Git data via TanStack Query and Tauri commands.
- **Hooks** (`hooks/use-*.ts`) — Public API for components and routes. Prefer hooks over importing stores or domain modules directly.

## Routes vs features

TanStack Router treats every file under `src/routes/` as a potential route module.

- **`routes/`** — Route definitions only (`route.tsx`, `index.tsx`, layout shells). No shared components, hooks, or utils here.
- **`features/<name>/`** — Route-specific UI and helpers (e.g. `features/git/components`, `features/git/lib`).
- **`components/`** — Shared UI used across multiple routes/features.

Do not put `components/` or `lib/` folders inside `routes/`. That triggers build warnings and couples non-route code to the router file scanner.

## Tooling

- **Format & lint:** Biome (`bun run lint`, `bun run format`)
- **Typecheck:** `tsc --noEmit` per package
- **UI components:** Import from `@gitru/ui/components/*`, utilities from `@gitru/ui/lib/utils`

## Rust (Tauri)

- Modules and files: snake_case (`branch.rs`, `repo_context_registry.rs`)
- Commands live in `apps/desktop/src-tauri/src/commands/`
