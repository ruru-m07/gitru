/**
 * React hooks for state domains
 * These hooks provide a clean DX for consuming state in components
 * It just Works goood!
 *
 * Usage:
 *   const { data, isLoading } = useStatus();
 *   const { data: branches } = useBranches();
 *   const { data: currentBranch } = useCurrentBranch();
 */

export * from "./useRepository";
