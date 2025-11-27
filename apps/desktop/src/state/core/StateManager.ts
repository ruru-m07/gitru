import { QueryClient } from "@tanstack/react-query";

/**
 * Centralized Query Client for the application state management.
 * We are using tanstack's react query for state invalidation and caching.
 * This client is shared across all state **domains**.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5,
      gcTime: 1000 * 60 * 30,
      retry: 2,
      refetchOnWindowFocus: false,
    },
  },
});

/**
 * Base class for all state domains.
 * Provides common utilities for query key management and invalidation.
 * More like a Base of all state domains.
 */
export abstract class StateDomain {
  constructor(protected queryClient: QueryClient) {}

  protected getQueryKey(...keys: string[]): string[] {
    return [this.constructor.name, ...keys];
  }
}
