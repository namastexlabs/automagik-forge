import React, { createContext, useContext, useEffect, useState } from 'react';
import type { User, UserSession } from 'shared/types';
import { authApi } from '@/lib/api';

interface AuthContextType {
  user: User | null;
  session: UserSession | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (token: string, user: User, session: UserSession) => void;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}

interface AuthProviderProps {
  children: React.ReactNode;
}

export function AuthProvider({ children }: AuthProviderProps) {
  const [user, setUser] = useState<User | null>(null);
  const [session, setSession] = useState<UserSession | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const isAuthenticated = !!user && !!session;

  const login = (token: string, userData: User, sessionData: UserSession) => {
    localStorage.setItem('auth_token', token);
    setUser(userData);
    setSession(sessionData);
  };

  const logout = async () => {
    try {
      await authApi.logout();
    } catch (error) {
      console.error('Logout API call failed:', error);
      // Continue with local logout even if API call fails
    } finally {
      localStorage.removeItem('auth_token');
      setUser(null);
      setSession(null);
    }
  };

  const refreshUser = async () => {
    try {
      const token = localStorage.getItem('auth_token');
      if (!token) {
        setIsLoading(false);
        return;
      }

      const response = await authApi.getCurrentUser();
      if (response.user && response.session) {
        setUser(response.user);
        setSession(response.session);
      } else {
        // Token invalid, clear it
        localStorage.removeItem('auth_token');
        setUser(null);
        setSession(null);
      }
    } catch (error) {
      console.error('Failed to refresh user info:', error);
      // Token likely invalid, clear it
      localStorage.removeItem('auth_token');
      setUser(null);
      setSession(null);
    } finally {
      setIsLoading(false);
    }
  };

  // Initialize authentication state on mount
  useEffect(() => {
    refreshUser();
  }, []);

  const value: AuthContextType = {
    user,
    session,
    isAuthenticated,
    isLoading,
    login,
    logout,
    refreshUser,
  };

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}