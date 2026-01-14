/**
 * SSE (Server-Sent Events) hooks and utilities
 *
 * This module provides React hooks for consuming SSE streams with
 * automatic reconnection using exponential backoff, last-event-ID
 * tracking for replay capability, and connection health monitoring.
 *
 * @example Basic usage
 * ```tsx
 * import { useSSEStream } from '@/hooks/sse';
 *
 * function MyComponent() {
 *   const { connectionState, isConnected, lastEventId } = useSSEStream(
 *     '/api/events/stream',
 *     true,
 *     {
 *       onMessage: (msg) => console.log('Received:', msg),
 *       maxBackoffMs: 30000,
 *     }
 *   );
 *
 *   return <div>Status: {connectionState}</div>;
 * }
 * ```
 *
 * @example With connection health indicator
 * ```tsx
 * import { useRealtimeConnection } from '@/hooks/sse';
 * import { ConnectionHealthIndicator } from '@/components/ui/ConnectionHealthIndicator';
 *
 * function RealtimePanel() {
 *   const connection = useRealtimeConnection('/api/events/stream', true);
 *
 *   return (
 *     <div>
 *       <ConnectionHealthIndicator
 *         state={connection.state}
 *         reconnectAttempts={connection.attemptCount}
 *         error={connection.error}
 *         showLabel
 *         onClick={connection.isDisconnected ? connection.reconnect : undefined}
 *       />
 *     </div>
 *   );
 * }
 * ```
 *
 * @example With context provider
 * ```tsx
 * import { SSEConnectionProvider, useSSEConnection } from '@/hooks/sse';
 *
 * // In parent component
 * <SSEConnectionProvider endpoint="/api/events/stream" enabled={true}>
 *   <App />
 * </SSEConnectionProvider>
 *
 * // In child component
 * function StatusBar() {
 *   const { connectionState, reconnect } = useSSEConnection();
 *   return <div>Status: {connectionState}</div>;
 * }
 * ```
 */

// Core SSE stream hook
export {
  useSSEStream,
  type SSEConnectionState,
  type SSEMessage,
  type UseSSEStreamOptions,
  type UseSSEStreamResult,
} from '../useSSEStream';

// High-level connection hook
export {
  useRealtimeConnection,
  type RealtimeConnectionStatus,
} from '../useRealtimeConnection';

// Context provider
export {
  SSEConnectionProvider,
  useSSEConnection,
  useSSEConnectionOptional,
  type SSEConnectionContextValue,
  type SSEConnectionProviderProps,
} from '../../contexts/sse-connection-context';
