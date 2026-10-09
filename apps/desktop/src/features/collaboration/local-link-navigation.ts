import type {
  LocalNavigationReceipt,
  LocalNavigationRequest,
} from "@gitru/collaboration-client";
import { defaultStringifySearch } from "@tanstack/react-router";

/** Persist IDs only; reload must resolve native authority again. */
export type LocalLinkRouteTarget = Omit<LocalNavigationReceipt, "selected"> & {
  link_id: string;
  generation: string;
};
const fields = [
  "local_repository_id",
  "link_id",
  "generation",
  "account_id",
  "instance_id",
  "repository_id",
  "authorization_epoch",
] as const;
export type LocalLinkSearch = Partial<LocalLinkRouteTarget> & {
  invalidLocalLink?: true;
};
export function parseLocalLinkSearch(
  search: Record<string, unknown>,
): LocalLinkSearch {
  if (search.invalidLocalLink === true) return { invalidLocalLink: true };
  if (!fields.some((key) => search[key] !== undefined)) return {};
  if (
    !fields.every(
      (key) =>
        typeof search[key] === "string" &&
        search[key].length > 0 &&
        search[key].length <= 1024 &&
        !search[key].includes("\0"),
    )
  )
    return { invalidLocalLink: true };
  return Object.fromEntries(
    fields.map((key) => [key, search[key]]),
  ) as LocalLinkRouteTarget;
}
export function localLinkTarget(
  search: LocalLinkSearch,
): LocalLinkRouteTarget | undefined {
  return !search.invalidLocalLink &&
    fields.every((key) => typeof search[key] === "string")
    ? (search as LocalLinkRouteTarget)
    : undefined;
}
export function localLinkSearch(target: LocalLinkRouteTarget) {
  return Object.fromEntries(fields.map((key) => [key, target[key]]));
}
export function localLinkRequest(
  target: LocalLinkRouteTarget,
): LocalNavigationRequest {
  return {
    local_repository_id: target.local_repository_id,
    link_id: target.link_id,
    generation: target.generation,
    direction: "collaboration",
  };
}
export function sameLocalLinkTarget(
  target: LocalLinkRouteTarget,
  receipt: LocalNavigationReceipt,
) {
  return (
    [
      "local_repository_id",
      "account_id",
      "instance_id",
      "repository_id",
      "authorization_epoch",
    ] as const
  ).every((key) => target[key] === receipt[key]);
}
export function localLinkRoutePath(
  kind: "pull_request" | "issue",
  target: LocalLinkRouteTarget,
) {
  // TanStack's decoder treats bare decimal URL values as JSON numbers. Its
  // serializer quotes opaque string IDs, including epochs above 2^53.
  return `/app/${kind === "issue" ? "issues" : "pulls"}${defaultStringifySearch(localLinkSearch(target))}`;
}
