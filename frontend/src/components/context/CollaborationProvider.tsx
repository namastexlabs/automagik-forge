import React, { createContext, useContext, useEffect, useState, useCallback, useRef } from 'react';
import { CollaborationEvent, UserPresence, PresenceStatus, PublicUser } from 'shared/types';
import { collaborationService, CollaborationServiceConfig } from '@/services/collaboration';
import { useAuth } from '@/components/auth-provider';

interface CollaborationContextType {
  // Connection state
  isConnected: boolean;
  isConnecting: boolean;
  connectionError: string | null;
  isOnline: boolean;
  
  // Presence data
  currentPresence: UserPresence[];
  onlineUsers: PublicUser[];
  
  // Event handling
  events: CollaborationEvent[];
  lastEvent: CollaborationEvent | null;
  
  // Actions
  connect: (projectId: string) => Promise<void>;
  disconnect: () => Promise<void>;
  updatePresence: (status: PresenceStatus) => Promise<void>;
  clearEvents: () => void;
  retry: () => Promise<void>;
  
  // Event subscription
  subscribeToEvents: (handler: (event: CollaborationEvent) => void) => () => void;
}

const CollaborationContext = createContext<CollaborationContextType | undefined>(undefined);

export function useCollaboration() {
  const context = useContext(CollaborationContext);
  if (context === undefined) {
    throw new Error('useCollaboration must be used within a CollaborationProvider');
  }
  return context;
}

interface CollaborationProviderProps {
  children: React.ReactNode;
}

export function CollaborationProvider({ children }: CollaborationProviderProps) {
  const { user, isAuthenticated } = useAuth();
  
  // Connection state
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [isOnline, setIsOnline] = useState(navigator.onLine);
  
  // Presence state
  const [currentPresence, setCurrentPresence] = useState<UserPresence[]>([]);
  
  // Event state
  const [events, setEvents] = useState<CollaborationEvent[]>([]);
  const [lastEvent, setLastEvent] = useState<CollaborationEvent | null>(null);
  
  // Event subscribers
  const eventSubscribersRef = useRef<Set<(event: CollaborationEvent) => void>>(new Set());
  
  // Current project ID
  const currentProjectIdRef = useRef<string | null>(null);

  // Derived state
  const onlineUsers = React.useMemo(() => {
    return currentPresence
      .filter(presence => presence.status === 'Online')
      .map(presence => ({
        id: presence.user_id,
        username: presence.username,
        display_name: presence.display_name,
        avatar_url: presence.avatar_url,
      }));
  }, [currentPresence]);

  // Handle collaboration events
  const handleCollaborationEvent = useCallback((event: CollaborationEvent) => {
    console.log('[CollaborationProvider] Received event:', event);
    
    // Update events list (keep last 100 events)
    setEvents(prev => {
      const newEvents = [event, ...prev].slice(0, 100);
      return newEvents;
    });
    
    // Update last event
    setLastEvent(event);
    
    // Notify subscribers
    eventSubscribersRef.current.forEach(handler => {
      try {
        handler(event);
      } catch (error) {
        console.error('[CollaborationProvider] Event handler error:', error);
      }
    });
  }, []);

  // Handle presence updates
  const handlePresenceUpdate = useCallback((presence: UserPresence[]) => {
    console.log('[CollaborationProvider] Presence update:', presence);
    setCurrentPresence(presence);
  }, []);

  // Handle connection status changes
  const handleConnectionStatus = useCallback((connected: boolean, error?: string) => {
    console.log('[CollaborationProvider] Connection status:', { connected, error });
    setIsConnected(connected);
    setIsConnecting(false);
    setConnectionError(error || null);
  }, []);

  // Connect to collaboration service
  const connect = useCallback(async (projectId: string) => {
    if (!isAuthenticated || !user) {
      throw new Error('User must be authenticated to connect to collaboration service');
    }

    const token = localStorage.getItem('auth_token');
    if (!token) {
      throw new Error('No authentication token available');
    }

    // Don't reconnect if already connected to the same project
    if (currentProjectIdRef.current === projectId && isConnected) {
      return;
    }

    // Disconnect from previous project if connected
    if (currentProjectIdRef.current && currentProjectIdRef.current !== projectId) {
      await disconnect();
    }

    currentProjectIdRef.current = projectId;
    setIsConnecting(true);
    setConnectionError(null);

    const config: CollaborationServiceConfig = {
      projectId,
      authToken: token,
      onEvent: handleCollaborationEvent,
      onPresenceUpdate: handlePresenceUpdate,
      onConnectionStatus: handleConnectionStatus,
    };

    try {
      await collaborationService.connect(config);
    } catch (error) {
      setIsConnecting(false);
      const errorMessage = error instanceof Error ? error.message : 'Connection failed';
      setConnectionError(errorMessage);
      throw error;
    }
  }, [isAuthenticated, user, isConnected, handleCollaborationEvent, handlePresenceUpdate, handleConnectionStatus]);

  // Disconnect from collaboration service
  const disconnect = useCallback(async () => {
    currentProjectIdRef.current = null;
    setIsConnecting(false);
    await collaborationService.disconnect();
    
    // Clear state
    setCurrentPresence([]);
    setEvents([]);
    setLastEvent(null);
  }, []);

  // Update presence status
  const updatePresence = useCallback(async (status: PresenceStatus) => {
    if (!isConnected) {
      throw new Error('Not connected to collaboration service');
    }
    
    try {
      await collaborationService.updatePresence(status);
    } catch (error) {
      console.error('[CollaborationProvider] Failed to update presence:', error);
      throw error;
    }
  }, [isConnected]);

  // Clear events
  const clearEvents = useCallback(() => {
    setEvents([]);
    setLastEvent(null);
  }, []);

  // Retry connection
  const retry = useCallback(async () => {
    if (currentProjectIdRef.current) {
      await connect(currentProjectIdRef.current);
    }
  }, [connect]);

  // Subscribe to events
  const subscribeToEvents = useCallback((handler: (event: CollaborationEvent) => void) => {
    eventSubscribersRef.current.add(handler);
    
    // Return unsubscribe function
    return () => {
      eventSubscribersRef.current.delete(handler);
    };
  }, []);

  // Handle user authentication changes
  useEffect(() => {
    if (!isAuthenticated && isConnected) {
      disconnect();
    }
  }, [isAuthenticated, isConnected, disconnect]);

  // Monitor network status
  useEffect(() => {
    const handleOnline = () => {
      console.log('[CollaborationProvider] Network back online');
      setIsOnline(true);
      // Attempt to reconnect if we have a project
      if (currentProjectIdRef.current && isAuthenticated && !isConnected) {
        retry().catch(console.error);
      }
    };

    const handleOffline = () => {
      console.log('[CollaborationProvider] Network went offline');
      setIsOnline(false);
      setConnectionError('Device is offline');
    };

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, [isAuthenticated, isConnected, retry]);

  // Handle visibility changes (for presence updates)
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (!isConnected) return;

      if (document.hidden) {
        // User switched away from tab
        updatePresence('Away' as PresenceStatus).catch(console.error);
      } else {
        // User returned to tab
        updatePresence('Online' as PresenceStatus).catch(console.error);
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, [isConnected, updatePresence]);

  // Handle beforeunload (cleanup on page close)
  useEffect(() => {
    const handleBeforeUnload = () => {
      if (isConnected) {
        // Update to offline status before closing
        updatePresence('Offline' as PresenceStatus).catch(console.error);
      }
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    
    return () => {
      window.removeEventListener('beforeunload', handleBeforeUnload);
    };
  }, [isConnected, updatePresence]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      collaborationService.destroy();
    };
  }, []);

  const value: CollaborationContextType = {
    // Connection state
    isConnected,
    isConnecting,
    connectionError,
    isOnline,
    
    // Presence data
    currentPresence,
    onlineUsers,
    
    // Event handling
    events,
    lastEvent,
    
    // Actions
    connect,
    disconnect,
    updatePresence,
    clearEvents,
    retry,
    subscribeToEvents,
  };

  return (
    <CollaborationContext.Provider value={value}>
      {children}
    </CollaborationContext.Provider>
  );
}