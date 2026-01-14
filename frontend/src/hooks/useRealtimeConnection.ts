import { useMemo } from 'react';
import { useSSEStream, type SSEConnectionState, type UseSSEStreamOptions } from './useSSEStream';

/**
 * Connection status summary for UI consumption
 */
export interface RealtimeConnectionStatus {
  /**
   * Raw connection state
   */
  state: SSEConnectionState;
  /**
   * Human-readable status label
   */
  label: string;
  /**
   * Whether connection is healthy
   */
  isHealthy: boolean;
  /**
   * Whether currently trying to connect
   */
  isConnecting: boolean;
  /**
   * Whether connection is broken
   */
  isDisconnected: boolean;
  /**
   * Error message if any
   */
  error: string | null;
  /**
   * Reconnection attempt count
   */
  attemptCount: number;
  /**
   * Last received event ID for replay
   */
  lastEventId: string | null;
  /**
   * Manual reconnect function
   */
  reconnect: () => void;
  /**
   * Manual disconnect function
   */
  disconnect: () => void;
}

const stateLabels: Record<SSEConnectionState, string> = {
  connected: 'Connected',
  connecting: 'Connecting...',
  reconnecting: 'Reconnecting...',
  disconnected: 'Disconnected',
};

/**
 * High-level hook for managing real-time SSE connections with
 * status tracking and automatic reconnection.
 *
 * This hook wraps useSSEStream and provides a simplified interface
 * suitable for UI consumption, including human-readable status labels
 * and convenience boolean flags.
 *
 * @param endpoint - The SSE endpoint URL
 * @param enabled - Whether the connection should be active
 * @param options - SSE stream configuration options
 * @returns Connection status and control functions
 *
 * @example
 * ```tsx
 * function RealtimePanel() {
 *   const [data, setData] = useState<MyData[]>([]);
 *
 *   const connection = useRealtimeConnection(
 *     '/api/events/stream',
 *     true,
 *     {
 *       onMessage: (msg) => {
 *         setData(prev => [...prev, msg.data]);
 *       },
 *       maxBackoffMs: 60000,
 *     }
 *   );
 *
 *   return (
 *     <div>
 *       <ConnectionHealthIndicator
 *         state={connection.state}
 *         reconnectAttempts={connection.attemptCount}
 *         error={connection.error}
 *         lastEventId={connection.lastEventId}
 *         onClick={connection.isDisconnected ? connection.reconnect : undefined}
 *       />
 *       {data.map(item => <DataItem key={item.id} data={item} />)}
 *     </div>
 *   );
 * }
 * ```
 */
export function useRealtimeConnection<T = unknown>(
  endpoint: string | undefined,
  enabled: boolean,
  options?: UseSSEStreamOptions<T>
): RealtimeConnectionStatus {
  const {
    connectionState,
    error,
    lastEventId,
    reconnectAttempts,
    reconnect,
    disconnect,
  } = useSSEStream(endpoint, enabled, options);

  const status = useMemo<RealtimeConnectionStatus>(() => ({
    state: connectionState,
    label: stateLabels[connectionState],
    isHealthy: connectionState === 'connected',
    isConnecting: connectionState === 'connecting' || connectionState === 'reconnecting',
    isDisconnected: connectionState === 'disconnected',
    error,
    attemptCount: reconnectAttempts,
    lastEventId,
    reconnect,
    disconnect,
  }), [connectionState, error, lastEventId, reconnectAttempts, reconnect, disconnect]);

  return status;
}

export default useRealtimeConnection;
