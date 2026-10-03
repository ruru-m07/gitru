import type { ContextualCapabilitySnapshot } from "@gitru/commands";
import type { Query, QueryClient } from "@tanstack/react-query";

/** One local-cache wakeup coordinator for the bridge; never performs provider HTTP. */
export function installCapabilityDeadlines(client: QueryClient) {
  // Weak keys follow cache eviction/account resets. Each query retains one
  // consumed deadline, rather than an unbounded history of snapshots or actors.
  const consumed = new WeakMap<Query, number>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stopped = false;

  const deadlines = () => {
    const result: { query: Query; deadline: number }[] = [];
    for (const query of client.getQueryCache().getAll()) {
      if (!isContext(query)) continue;
      const snapshot = query.state.data as
        | ContextualCapabilitySnapshot
        | undefined;
      if (!snapshot) continue;
      const dates = snapshot.facets.flatMap((facet) => {
        if (
          facet.synchronize.state !== "unavailable" ||
          facet.synchronize.reason !== "temporarily_unavailable"
        )
          return [];
        const date = Date.parse(facet.sync.next_retry_at ?? "");
        return Number.isFinite(date) ? [date] : [];
      });
      if (dates.length === 0) continue;
      const deadline = Math.min(...dates);
      if (consumed.get(query) !== deadline) result.push({ query, deadline });
    }
    return result;
  };

  const schedule = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    if (stopped) return;
    const dates = deadlines();
    if (!dates.length) return;
    const delay = Math.max(
      0,
      Math.min(...dates.map((entry) => entry.deadline)) - Date.now(),
    );
    // setTimeout overflows at ~24.8 days; a distant wake only rescans locally.
    timer = setTimeout(
      () => {
        void expire();
      },
      Math.min(delay, 2_147_483_647),
    );
  };

  const expire = async () => {
    const expired = deadlines().filter((entry) => entry.deadline <= Date.now());
    for (const entry of expired) consumed.set(entry.query, entry.deadline);
    // Mark first: cache events during cancellation/refetch cannot invalidate
    // the same unchanged deadline repeatedly while its replacement is pending.
    await Promise.allSettled(
      expired.map(async ({ query }) => {
        const filter = { queryKey: query.queryKey, exact: true };
        await client.cancelQueries(filter);
        if (!stopped && client.getQueryCache().get(query.queryHash) === query)
          await client.invalidateQueries(filter);
      }),
    );
    schedule();
  };

  const unsubscribe = client.getQueryCache().subscribe((event) => {
    if (isContext(event.query)) schedule();
  });
  schedule();
  return () => {
    stopped = true;
    unsubscribe();
    if (timer !== undefined) clearTimeout(timer);
  };
}

function isContext(query: Query) {
  return (
    query.queryKey[0] === "collaboration" &&
    query.queryKey[1] === "account" &&
    query.queryKey[4] === "capabilities" &&
    query.queryKey[5] === "context"
  );
}
