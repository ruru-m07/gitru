export type Distribution = {
  count: number;
  minimum: number;
  p50: number;
  p95: number;
  p99: number;
  maximum: number;
};

export function nearestRank(samples: readonly number[], percentile: number) {
  if (
    samples.length === 0 ||
    !Number.isFinite(percentile) ||
    percentile <= 0 ||
    percentile > 1 ||
    samples.some((sample) => !Number.isFinite(sample) || sample < 0)
  )
    throw new Error("Performance samples must be finite and non-negative");
  const sorted = [...samples].sort((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(percentile * sorted.length) - 1)];
}

export function distribution(samples: readonly number[]): Distribution {
  return {
    count: samples.length,
    minimum: nearestRank(samples, 1 / samples.length),
    p50: nearestRank(samples, 0.5),
    p95: nearestRank(samples, 0.95),
    p99: nearestRank(samples, 0.99),
    maximum: nearestRank(samples, 1),
  };
}
