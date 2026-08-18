/**
 * Type definitions for React Dashboard component and custom hooks.
 */

export interface MetricItem {
  id: string;
  label: string;
  value: number;
  changeRate: number;
  category: 'revenue' | 'traffic' | 'conversion' | 'latency';
  timestamp: string;
}

export interface UserSession {
  userId: string;
  email: string;
  role: 'admin' | 'editor' | 'viewer';
  permissions: string[];
  lastLogin: Date;
}

export interface FilterState {
  category?: string;
  startDate?: string;
  endDate?: string;
  searchQuery: string;
  sortBy: 'date' | 'value' | 'name';
  sortOrder: 'asc' | 'desc';
}

export interface DashboardProps {
  title: string;
  initialFilters?: Partial<FilterState>;
  onExportReport?: (filters: FilterState) => Promise<void>;
  enableLiveUpdates?: boolean;
  refreshIntervalMs?: number;
}
