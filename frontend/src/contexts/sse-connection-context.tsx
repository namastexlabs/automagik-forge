import React, { createContext, useContext, useMemo } from 'react';
import { useSSEStream, type SSEConnectionState, type UseSSEStreamOptions } from '@/hooks/useSSEStream';

/**
 * SSE Connection context value
 */
export interface SSEConnectionContextValue {
  /**
   * Current connection state
   */
  connectionState: SSEConnectionState;
  /**
   * Whether the stream is connected
   */
  isConnected: boolean;
  /**
   * Current error message, if any
   */
  error: string | null;
  /**
   * Last event ID received (for replay capability)
   */
  lastEventId: string | null;
  /**
   * Number of reconnection attempts
   */
  reconnectAttempts: number;
  /**
   * Manually reconnect the stream
   */
  reconnect: () => void;
  /**
   * Manually disconnect the stream
   */
  disconnect: () => void;
}

const SSEConnectionContext = createContext<SSEConnectionContextValue | null>(null);

export interface SSEConnectionProviderProps<T = unknown> {
  /**
   * The SSE endpoint URL
   */
  endpoint: string | undefined;
  /**
   * Whether the stream should be active
   */
  enabled?: boolean;
  /**
   * SSE stream options
   */
  options?: UseSSEStreamOptions<T>;
  /**
   * Child components
   */
  children: React.ReactNode;
}

/**
 * Provider component for SSE connection state.
 * Wraps children with SSE connection context, making connection
 * state available to all descendants.
 *
 * @example
 * ```tsx
 * <SSEConnectionProvider
 *   endpoint="/api/events/stream"
 *   enabled={true}
 *   options={{
 *     onMessage: (msg) => handleMessage(msg),
 *     maxBackoffMs: 30000,
 *   }}
 * >
 *   <App />
 * </SSEConnectionProvider>
 * ```
 */
export function SSEConnectionProvider<T = unknown>({
  endpoint,
  enabled = true,
  options,
  children,
}: SSEConnectionProviderProps<T>) {
  const {
    connectionState,
    isConnected,
    error,
    lastEventId,
    reconnectAttempts,
    reconnect,
    disconnect,
  } = useSSEStream(endpoint, enabled, options);

  const value = useMemo<SSEConnectionContextValue>(
    () => ({
      connectionState,
      isConnected,
      error,
      lastEventId,
      reconnectAttempts,
      reconnect,
      disconnect,
    }),
    [connectionState, isConnected, error, lastEventId, reconnectAttempts, reconnect, disconnect]
  );

  return (
    <SSEConnectionContext.Provider value={value}>
      {children}
    </SSEConnectionContext.Provider>
  );
}

/**
 * Hook to access SSE connection state from context.
 * Must be used within an SSEConnectionProvider.
 *
 * @throws Error if used outside of SSEConnectionProvider
 *
 * @example
 * ```tsx
 * function MyComponent() {
 *   const { connectionState, isConnected, reconnect } = useSSEConnection();
 *
 *   return (
 *     <ConnectionHealthIndicator
 *       state={connectionState}
 *       onClick={reconnect}
 *     />
 *   );
 * }
 * ```
 */
export function useSSEConnection(): SSEConnectionContextValue {
  const context = useContext(SSEConnectionContext);

  if (!context) {
    throw new Error('useSSEConnection must be used within an SSEConnectionProvider');
  }

  return context;
}

/**
 * Hook to optionally access SSE connection state from context.
 * Returns null if not within an SSEConnectionProvider.
 * Useful for components that may or may not be within an SSE context.
 *
 * @example
 * ```tsx
 * function OptionalIndicator() {
 *   const connection = useSSEConnectionOptional();
 *
 *   if (!connection) return null;
 *
 *   return (
 *     <ConnectionHealthIndicator state={connection.connectionState} />
 *   );
 * }
 * ```
 */
export function useSSEConnectionOptional(): SSEConnectionContextValue | null {
  return useContext(SSEConnectionContext);
}

export default SSEConnectionContext;
