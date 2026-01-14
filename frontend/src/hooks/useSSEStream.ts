import { useEffect, useState, useRef, useCallback } from 'react';

/**
 * Connection states for SSE stream
 */
export type SSEConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

/**
 * SSE message event with optional ID for replay capability
 */
export interface SSEMessage<T = unknown> {
  id?: string;
  event?: string;
  data: T;
}

/**
 * Options for the SSE stream hook
 */
export interface UseSSEStreamOptions<T> {
  /**
   * Called when a message is received
   */
  onMessage?: (message: SSEMessage<T>) => void;
  /**
   * Called when connection state changes
   */
  onStateChange?: (state: SSEConnectionState) => void;
  /**
   * Called on error
   */
  onError?: (error: Error) => void;
  /**
   * Initial backoff delay in milliseconds (default: 1000)
   */
  initialBackoffMs?: number;
  /**
   * Maximum backoff delay in milliseconds (default: 30000)
   */
  maxBackoffMs?: number;
  /**
   * Backoff multiplier (default: 2)
   */
  backoffMultiplier?: number;
  /**
   * Maximum number of reconnection attempts (default: Infinity)
   */
  maxReconnectAttempts?: number;
  /**
   * Custom headers to send with the request
   */
  headers?: Record<string, string>;
  /**
   * Whether to include credentials (default: false)
   */
  withCredentials?: boolean;
}

/**
 * Result from the SSE stream hook
 */
export interface UseSSEStreamResult {
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

/**
 * Calculate exponential backoff delay with jitter
 */
function calculateBackoffDelay(
  attempt: number,
  initialMs: number,
  maxMs: number,
  multiplier: number
): number {
  // Exponential backoff: initialMs * multiplier^attempt
  const exponentialDelay = initialMs * Math.pow(multiplier, attempt);
  // Cap at maximum
  const cappedDelay = Math.min(exponentialDelay, maxMs);
  // Add jitter (±10%) to prevent thundering herd
  const jitter = cappedDelay * 0.1 * (Math.random() * 2 - 1);
  return Math.round(cappedDelay + jitter);
}

/**
 * Hook for consuming Server-Sent Events (SSE) streams with automatic
 * reconnection using exponential backoff and last-event-ID tracking
 * for replay capability.
 *
 * @param endpoint - The SSE endpoint URL
 * @param enabled - Whether the stream should be active
 * @param options - Configuration options
 * @returns SSE stream state and controls
 *
 * @example
 * ```tsx
 * const { connectionState, isConnected, lastEventId, reconnect } = useSSEStream(
 *   '/api/events/stream',
 *   true,
 *   {
 *     onMessage: (msg) => console.log('Received:', msg),
 *     onStateChange: (state) => console.log('State:', state),
 *     maxBackoffMs: 30000,
 *   }
 * );
 * ```
 */
export function useSSEStream<T = unknown>(
  endpoint: string | undefined,
  enabled: boolean,
  options: UseSSEStreamOptions<T> = {}
): UseSSEStreamResult {
  const {
    onMessage,
    onStateChange,
    onError,
    initialBackoffMs = 1000,
    maxBackoffMs = 30000,
    backoffMultiplier = 2,
    maxReconnectAttempts = Infinity,
    // Note: headers is available for future extension when using fetch + ReadableStream
    // Native EventSource doesn't support custom headers
    headers: _headers,
    withCredentials = false,
  } = options;

  const [connectionState, setConnectionState] = useState<SSEConnectionState>('disconnected');
  const [error, setError] = useState<string | null>(null);
  const [lastEventId, setLastEventId] = useState<string | null>(null);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);
  const [reconnectTrigger, setReconnectTrigger] = useState(0);

  // Refs for managing connection lifecycle
  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimerRef = useRef<number | null>(null);
  const intentionalCloseRef = useRef(false);
  const mountedRef = useRef(true);
  const reconnectAttemptsRef = useRef(0);

  // Store callbacks in refs to avoid effect dependencies
  const onMessageRef = useRef(onMessage);
  const onStateChangeRef = useRef(onStateChange);
  const onErrorRef = useRef(onError);

  useEffect(() => {
    onMessageRef.current = onMessage;
    onStateChangeRef.current = onStateChange;
    onErrorRef.current = onError;
  }, [onMessage, onStateChange, onError]);

  // Reset lastEventId when endpoint changes (Issue #3)
  useEffect(() => {
    setLastEventId(null);
  }, [endpoint]);

  // Update connection state and notify callback
  const updateConnectionState = useCallback((newState: SSEConnectionState) => {
    if (!mountedRef.current) return;
    setConnectionState(newState);
    onStateChangeRef.current?.(newState);
  }, []);

  // Schedule a reconnection attempt
  const scheduleReconnect = useCallback(() => {
    if (!mountedRef.current) return;
    if (reconnectTimerRef.current) return; // Already scheduled

    const currentAttempt = reconnectAttemptsRef.current;
    if (currentAttempt >= maxReconnectAttempts) {
      setError(`Max reconnection attempts (${maxReconnectAttempts}) reached`);
      updateConnectionState('disconnected');
      return;
    }

    const delay = calculateBackoffDelay(
      currentAttempt,
      initialBackoffMs,
      maxBackoffMs,
      backoffMultiplier
    );

    console.log(`[SSE] Scheduling reconnect attempt ${currentAttempt + 1} in ${delay}ms`);

    reconnectTimerRef.current = window.setTimeout(() => {
      reconnectTimerRef.current = null;
      if (mountedRef.current) {
        reconnectAttemptsRef.current += 1;
        setReconnectAttempts(reconnectAttemptsRef.current);
        setReconnectTrigger(prev => prev + 1); // Trigger reconnection
      }
    }, delay);
  }, [maxReconnectAttempts, initialBackoffMs, maxBackoffMs, backoffMultiplier, updateConnectionState]);

  // Clean up existing connection
  const cleanup = useCallback(() => {
    if (reconnectTimerRef.current) {
      window.clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
  }, []);

  // Manual disconnect
  const disconnect = useCallback(() => {
    intentionalCloseRef.current = true;
    cleanup();
    updateConnectionState('disconnected');
  }, [cleanup, updateConnectionState]);

  // Manual reconnect
  const reconnect = useCallback(() => {
    intentionalCloseRef.current = false;
    cleanup();
    reconnectAttemptsRef.current = 0;
    setReconnectAttempts(0);
    setError(null);
    // Trigger reconnection
    setReconnectTrigger(prev => prev + 1);
  }, [cleanup]);

  // Main effect for managing SSE connection
  useEffect(() => {
    mountedRef.current = true;

    if (!enabled || !endpoint) {
      cleanup();
      updateConnectionState('disconnected');
      setError(null);
      reconnectAttemptsRef.current = 0;
      setReconnectAttempts(0);
      return;
    }

    // Don't create new connection if intentionally closed
    if (intentionalCloseRef.current) {
      return;
    }

    // Already have a connection
    if (eventSourceRef.current) {
      return;
    }

    // Build URL with last-event-id for replay capability
    const url = new URL(endpoint, window.location.origin);
    if (lastEventId) {
      url.searchParams.set('lastEventId', lastEventId);
    }

    updateConnectionState(reconnectAttemptsRef.current > 0 ? 'reconnecting' : 'connecting');

    // Create EventSource
    // Note: Native EventSource doesn't support custom headers
    // For headers support, we'd need to use fetch + ReadableStream
    // or a polyfill like eventsource-polyfill
    let eventSource: EventSource;

    try {
      eventSource = new EventSource(url.toString(), {
        withCredentials,
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to create EventSource';
      setError(errorMsg);
      onErrorRef.current?.(new Error(errorMsg));
      updateConnectionState('disconnected');
      return;
    }

    eventSourceRef.current = eventSource;

    // Handle successful connection
    eventSource.onopen = () => {
      if (!mountedRef.current) return;
      console.log('[SSE] Connected');
      setError(null);
      // Reset reconnect counter using ref to avoid triggering effect rerun (Issue #2)
      reconnectAttemptsRef.current = 0;
      setReconnectAttempts(0);
      updateConnectionState('connected');
    };

    // Handle incoming messages
    eventSource.onmessage = (event: MessageEvent) => {
      if (!mountedRef.current) return;

      try {
        // Track last event ID for replay capability
        if (event.lastEventId) {
          setLastEventId(event.lastEventId);
        }

        // Parse message data
        let data: T;
        try {
          data = JSON.parse(event.data);
        } catch {
          // If not JSON, use raw data
          data = event.data as T;
        }

        const message: SSEMessage<T> = {
          id: event.lastEventId || undefined,
          data,
        };

        onMessageRef.current?.(message);
      } catch (err) {
        console.error('[SSE] Failed to process message:', err);
      }
    };

    // Handle errors
    eventSource.onerror = (event: Event) => {
      if (!mountedRef.current) return;

      console.error('[SSE] Connection error:', event);

      // EventSource will automatically try to reconnect on error
      // but we want to use our own exponential backoff strategy
      eventSource.close();
      eventSourceRef.current = null;

      if (intentionalCloseRef.current) {
        updateConnectionState('disconnected');
        return;
      }

      const errorMsg = 'Connection lost';
      setError(errorMsg);
      onErrorRef.current?.(new Error(errorMsg));

      // Schedule reconnection with exponential backoff
      scheduleReconnect();
    };

    return () => {
      mountedRef.current = false;
      cleanup();
    };
  }, [
    endpoint,
    enabled,
    reconnectTrigger, // Triggers reconnection without causing issues
    // Note: lastEventId removed from deps to avoid reconnect loop (Issue #1)
    // Note: reconnectAttempts removed from deps to avoid closing connection on success (Issue #2)
    withCredentials,
    cleanup,
    updateConnectionState,
    scheduleReconnect,
  ]);

  return {
    connectionState,
    isConnected: connectionState === 'connected',
    error,
    lastEventId,
    reconnectAttempts,
    reconnect,
    disconnect,
  };
}

export default useSSEStream;
