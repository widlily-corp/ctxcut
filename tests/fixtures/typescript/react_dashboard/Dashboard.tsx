import React, { useState } from 'react';
import { DashboardProps, FilterState, MetricItem } from './types';
import { useMetrics } from './useMetrics';
import { useAuth } from './useAuth';

export function Dashboard(props: DashboardProps): JSX.Element {
  const {
    title,
    initialFilters = { searchQuery: '', sortBy: 'date', sortOrder: 'desc' },
    onExportReport,
    refreshIntervalMs,
  } = props;

  const [filters, setFilters] = useState<FilterState>({
    searchQuery: initialFilters.searchQuery || '',
    sortBy: initialFilters.sortBy || 'date',
    sortOrder: initialFilters.sortOrder || 'desc',
    category: initialFilters.category,
  });

  const [isFilterDrawerOpen, setIsFilterDrawerOpen] = useState(false);
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const [selectedMetric, setSelectedMetric] = useState<MetricItem | null>(null);

  const { metrics, isLoading, error, refresh } = useMetrics(filters, refreshIntervalMs);
  const { currentUser, hasPermission, logout } = useAuth();

  return (
    <div className="dashboard-container min-h-screen bg-slate-900 text-slate-100 p-6">
      {/* Top Header Bar */}
      <header className="flex justify-between items-center pb-6 border-b border-slate-800">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{title}</h1>
          <p className="text-sm text-slate-400">
            Welcome back, {currentUser?.email ?? 'Guest'} ({currentUser?.role ?? 'viewer'})
          </p>
        </div>
        <div className="flex gap-3">
          <button
            onClick={() => setIsFilterDrawerOpen(true)}
            className="px-4 py-2 bg-slate-800 hover:bg-slate-700 rounded-md text-sm"
          >
            Filters
          </button>
          {hasPermission('metrics:export') && (
            <button
              onClick={() => setIsExportModalOpen(true)}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-md text-sm font-medium"
            >
              Export Report
            </button>
          )}
          <button
            onClick={refresh}
            className="px-4 py-2 border border-slate-700 hover:bg-slate-800 rounded-md text-sm"
          >
            Refresh
          </button>
        </div>
      </header>

      {/* Main Metric Cards Grid */}
      <main className="mt-6">
        {isLoading ? (
          <div className="text-center py-12 text-slate-400">Loading live metrics...</div>
        ) : error ? (
          <div className="text-red-400 p-4 bg-red-950/40 rounded border border-red-800">
            Error loading metrics: {error.message}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {metrics.map((item) => (
              <div
                key={item.id}
                onClick={() => setSelectedMetric(item)}
                className="p-5 bg-slate-800/60 hover:bg-slate-800 border border-slate-700/60 rounded-lg cursor-pointer"
              >
                <span className="text-xs uppercase tracking-wider text-slate-400 font-mono">
                  {item.category}
                </span>
                <h3 className="text-lg font-semibold mt-1">{item.label}</h3>
                <div className="text-3xl font-mono font-bold mt-2 tabular-nums">
                  {typeof item.value === 'number' ? item.value.toLocaleString() : item.value}
                </div>
                <div className="text-xs mt-2 text-emerald-400">
                  {item.changeRate > 0 ? `+${(item.changeRate * 100).toFixed(1)}%` : `${(item.changeRate * 100).toFixed(1)}%`} from last period
                </div>
              </div>
            ))}
          </div>
        )}
      </main>

      {/* Secondary Collapsible Branch: Filter Drawer */}
      {isFilterDrawerOpen && (
        <aside className="fixed inset-y-0 right-0 w-80 bg-slate-800 border-l border-slate-700 p-6 z-50 shadow-2xl">
          <div className="flex justify-between items-center mb-6">
            <h2 className="text-lg font-semibold">Filter Metrics</h2>
            <button
              onClick={() => setIsFilterDrawerOpen(false)}
              className="text-slate-400 hover:text-slate-200"
            >
              Close
            </button>
          </div>
          <div className="space-y-4">
            <div>
              <label className="block text-xs uppercase text-slate-400 mb-1">Search</label>
              <input
                type="text"
                value={filters.searchQuery}
                onChange={(e) => setFilters({ ...filters, searchQuery: e.target.value })}
                className="w-full bg-slate-900 border border-slate-700 rounded px-3 py-2 text-sm"
                placeholder="Search metrics..."
              />
            </div>
            <div>
              <label className="block text-xs uppercase text-slate-400 mb-1">Sort By</label>
              <select
                value={filters.sortBy}
                onChange={(e) =>
                  setFilters({ ...filters, sortBy: e.target.value as FilterState['sortBy'] })
                }
                className="w-full bg-slate-900 border border-slate-700 rounded px-3 py-2 text-sm"
              >
                <option value="date">Date</option>
                <option value="value">Value</option>
                <option value="name">Name</option>
              </select>
            </div>
          </div>
        </aside>
      )}

      {/* Secondary Collapsible Branch: Details Sidebar */}
      {selectedMetric && (
        <div className="fixed bottom-6 right-6 w-96 bg-slate-800 border border-slate-700 rounded-lg p-5 shadow-xl">
          <div className="flex justify-between items-center mb-3">
            <h3 className="font-semibold">{selectedMetric.label}</h3>
            <button
              onClick={() => setSelectedMetric(null)}
              className="text-slate-400 hover:text-slate-200 text-sm"
            >
              Dismiss
            </button>
          </div>
          <p className="text-sm text-slate-300">ID: {selectedMetric.id}</p>
          <p className="text-sm text-slate-300">Category: {selectedMetric.category}</p>
          <p className="text-sm text-slate-300">Timestamp: {selectedMetric.timestamp}</p>
        </div>
      )}

      {/* Secondary Collapsible Branch: Export Modal Dialog */}
      {isExportModalOpen && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center p-4 z-50">
          <div className="bg-slate-800 border border-slate-700 rounded-lg p-6 max-w-md w-full">
            <h2 className="text-lg font-bold mb-3">Export Analytics Report</h2>
            <p className="text-sm text-slate-300 mb-4">
              Generate and download a CSV or PDF summary of all current telemetry metrics.
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setIsExportModalOpen(false)}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded text-sm"
              >
                Cancel
              </button>
              <button
                onClick={async () => {
                  if (onExportReport) {
                    await onExportReport(filters);
                  }
                  setIsExportModalOpen(false);
                }}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded text-sm font-medium"
              >
                Confirm Export
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
