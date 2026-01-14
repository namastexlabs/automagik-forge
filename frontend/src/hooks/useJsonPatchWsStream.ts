import { useEffect, useState, useRef, useId } from 'react';
import { applyPatch } from 'rfc6902';
import type { Operation } from 'rfc6902';
import { useGlobalStreamStatus } from './useGlobalStreamStatus';

type WsJsonPatchMsg = { JsonPatch: Operation[] };
type WsFinishedMsg = { finished: boolean };
type WsMsg = WsJsonPatchMsg | WsFinishedMsg;

interface UseJsonPatchStreamOptions<T> {
  /**
   * Called once when the stream starts to inject initial data
   */
  injectInitialEntry?: (data: T) => void;
  /**
   * Filter/deduplicate patches before applying them
   */
  deduplicatePatches?: (patches: Operation[]) => Operation[];
}

interface UseJsonPatchStreamResult<T> {
  data: T | undefined;
  isConnected: boolean;
  error: string | null;
}

/**
 * Generic hook for consuming WebSocket streams that send JSON messages with patches
 */
export const useJsonPatchWsStream = <T>(
  endpoint: string | undefined,
  enabled: boolean,
  initialData: () => T,
  options: UseJsonPatchStreamOptions<T> = {}
): UseJsonPatchStreamResult<T> => {
  const [data, setData] = useState<T | undefined>(undefined);
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const dataRef = useRef<T | undefined>(undefined);
  const retryTimerRef = useRef<number | null>(null);
  const retryAttemptsRef = useRef<number>(0);
  const [retryNonce, setRetryNonce] = useState(0);
  const finishedRef = useRef<boolean>(false);

  // Global stream status tracking
  const streamId = useId();
  const updateStream = useGlobalStreamStatus((s) => s.updateStream);
  const removeStream = useGlobalStreamStatus((s) => s.removeStream);

  // Register/update stream status with global tracker
  useEffect(() => {
    if (enabled && endpoint) {
      // Extract a friendly name from the endpoint
      const streamName = endpoint.includes('tasks/stream')
        ? 'Tasks'
        : endpoint.includes('diff/ws')
          ? 'Diff'
          : endpoint.includes('logs/ws')
            ? 'Logs'
            : endpoint.includes('execution-processes')
              ? 'Processes'
              : endpoint.includes('drafts/stream')
                ? 'Drafts'
                : 'Stream';
      updateStream(streamId, streamName, isConnected, error);
    }
    return () => {
      removeStream(streamId);
    };
  }, [streamId, endpoint, enabled, isConnected, error, updateStream, removeStream]);

  function scheduleReconnect() {
    if (retryTimerRef.current) return; // already scheduled
    // Exponential backoff with cap: 1s, 2s, 4s, 8s (max), then stay at 8s
    const attempt = retryAttemptsRef.current;
    const delay = Math.min(8000, 1000 * Math.pow(2, attempt));
    retryTimerRef.current = window.setTimeout(() => {
      retryTimerRef.current = null;
      setRetryNonce((n) => n + 1);
    }, delay);
  }

  // Store options in a ref to avoid recreating WebSocket on every options change
  const optionsRef = useRef(options);
  useEffect(() => {
    optionsRef.current = options;
  }, [options]);

  useEffect(() => {
    if (!enabled || !endpoint) {
      // Close connection and reset state
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
      if (retryTimerRef.current) {
        window.clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      retryAttemptsRef.current = 0;
      finishedRef.current = false;
      setData(undefined);
      setIsConnected(false);
      setError(null);
      dataRef.current = undefined;
      return;
    }

    // Initialize data
    if (!dataRef.current) {
      dataRef.current = initialData();

      // Inject initial entry if provided
      if (optionsRef.current.injectInitialEntry) {
        optionsRef.current.injectInitialEntry(dataRef.current);
      }
    }

    // Create WebSocket if it doesn't exist
    if (!wsRef.current) {
      // Reset finished flag for new connection
      finishedRef.current = false;

      // Convert HTTP endpoint to WebSocket endpoint
      const wsEndpoint = endpoint.replace(/^http/, 'ws');
      const ws = new WebSocket(wsEndpoint);

      ws.onopen = () => {
        setError(null);
        setIsConnected(true);
        // Reset backoff on successful connection
        retryAttemptsRef.current = 0;
        if (retryTimerRef.current) {
          window.clearTimeout(retryTimerRef.current);
          retryTimerRef.current = null;
        }
      };

      ws.onmessage = (event) => {
        try {
          const msg: WsMsg = JSON.parse(event.data);

          // Handle JsonPatch messages (same as SSE json_patch event)
          if ('JsonPatch' in msg) {
            const patches: Operation[] = msg.JsonPatch;
            const filtered = optionsRef.current.deduplicatePatches
              ? optionsRef.current.deduplicatePatches(patches)
              : patches;

            if (!filtered.length || !dataRef.current) return;

            // Deep clone the current state before mutating it
            dataRef.current = structuredClone(dataRef.current);

            // Apply patch (mutates the clone in place)
            applyPatch(dataRef.current as object, filtered);

            // React re-render: dataRef.current is already a new object
            setData(dataRef.current);
          }

          // Handle finished messages ({finished: true})
          // Treat finished as terminal - do NOT reconnect
          if ('finished' in msg) {
            finishedRef.current = true;
            ws.close(1000, 'finished');
            wsRef.current = null;
            setIsConnected(false);
          }
        } catch (err) {
          console.error('Failed to process WebSocket message:', err);
          setError('Failed to process stream update');
        }
      };

      ws.onerror = () => {
        setError('Connection failed');
      };

      ws.onclose = (evt) => {
        setIsConnected(false);
        wsRef.current = null;

        // Do not reconnect if we received a finished message or clean close
        if (finishedRef.current || (evt?.code === 1000 && evt?.wasClean)) {
          return;
        }

        // Otherwise, reconnect on unexpected/error closures
        retryAttemptsRef.current += 1;
        scheduleReconnect();
      };

      wsRef.current = ws;
    }

    return () => {
      if (wsRef.current) {
        const ws = wsRef.current;

        // Clear event handlers to prevent callbacks after cleanup
        ws.onmessage = null;
        ws.onerror = null;
        ws.onclose = null;

        if (ws.readyState === WebSocket.CONNECTING) {
          ws.onopen = () => {
            ws.onopen = null;
            try {
              ws.close(1000, 'cleanup');
            } catch (e) {
              console.debug('WebSocket close during cleanup failed:', e);
            }
          };
        } else if (
          ws.readyState === WebSocket.OPEN ||
          ws.readyState === WebSocket.CLOSING
        ) {
          try {
            ws.close(1000, 'cleanup');
          } catch (e) {
            console.debug('WebSocket close during cleanup failed:', e);
          }
        }
        wsRef.current = null;
      }
      if (retryTimerRef.current) {
        window.clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      finishedRef.current = false;
      dataRef.current = undefined;
      setData(undefined);
    };
  }, [endpoint, enabled, initialData, retryNonce]);

  return { data, isConnected, error };
};
