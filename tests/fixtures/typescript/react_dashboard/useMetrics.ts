import { useState, useEffect, useCallback } from 'react';
import { MetricItem, FilterState } from './types';

export interface UseMetricsResult {
  metrics: MetricItem[];
  isLoading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
}

export function useMetrics(filters: FilterState, autoRefreshInterval?: number): UseMetricsResult {
  const [metrics, setMetrics] = useState<MetricItem[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchMetrics = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      // Simulated API fetch
      const sampleData: MetricItem[] = [
        {
          id: 'm-1',
          label: 'Monthly Recurring Revenue',
          value: 124500,
          changeRate: 0.125,
          category: 'revenue',
          timestamp: new Date().toISOString(),
        },
        {
          id: 'm-2',
          label: 'Daily Active Users',
          value: 48920,
          changeRate: 0.043,
          category: 'traffic',
          timestamp: new Date().toISOString(),
        },
        {
          id: 'm-3',
          label: 'P99 API Latency',
          value: 42.5,
          changeRate: -0.15,
          category: 'latency',
          timestamp: new Date().toISOString(),
        },
      ];
      setMetrics(sampleData);
    } catch (err) {
      setError(err instanceof Error ? err : new Error('Failed to load metrics'));
    } finally {
      setIsLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    fetchMetrics();

    if (autoRefreshInterval && autoRefreshInterval > 0) {
      const intervalId = setInterval(fetchMetrics, autoRefreshInterval);
      return () => clearInterval(intervalId);
    }
  }, [fetchMetrics, autoRefreshInterval]);

  return {
    metrics,
    isLoading,
    error,
    refresh: fetchMetrics,
  };
}
