import { useState, useCallback } from 'react';
import { UserSession } from './types';

export interface UseAuthResult {
  currentUser: UserSession | null;
  isAuthenticated: boolean;
  hasPermission: (permission: string) => boolean;
  logout: () => void;
}

export function useAuth(): UseAuthResult {
  const [currentUser] = useState<UserSession | null>({
    userId: 'usr-9041',
    email: 'architect@acme.corp',
    role: 'admin',
    permissions: ['metrics:read', 'metrics:export', 'settings:write'],
    lastLogin: new Date(),
  });

  const hasPermission = useCallback(
    (permission: string): boolean => {
      if (!currentUser) return false;
      if (currentUser.role === 'admin') return true;
      return currentUser.permissions.includes(permission);
    },
    [currentUser]
  );

  const logout = useCallback(() => {
    // Clear auth session logic
  }, []);

  return {
    currentUser,
    isAuthenticated: currentUser !== null,
    hasPermission,
    logout,
  };
}
