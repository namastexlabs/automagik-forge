import { create } from 'zustand';
import type { SSEConnectionState } from './useSSEStream';

/**
 * Stream registration entry
 */
interface StreamEntry {
  id: string;
  name: string;
  isConnected: boolean;
  error: string | null;
  lastUpdated: number;
}

/**
 * Global stream status store
 */
interface GlobalStreamStatusStore {
  /**
   * Registered streams
   */
  streams: Map<string, StreamEntry>;
  /**
   * Register or update a stream's status
   */
  updateStream: (id: string, name: string, isConnected: boolean, error: string | null) => void;
  /**
   * Remove a stream from tracking
   */
  removeStream: (id: string) => void;
  /**
   * Get overall connection state based on all streams
   */
  getOverallState: () => SSEConnectionState;
  /**
   * Get count of connected streams
   */
  getConnectedCount: () => number;
  /**
   * Get count of total streams
   */
  getTotalCount: () => number;
  /**
   * Check if any stream has an error
   */
  hasErrors: () => boolean;
  /**
   * Get first error message
   */
  getFirstError: () => string | null;
}

/**
 * Global stream status store using Zustand
 * Tracks multiple real-time connections and provides aggregate status
 */
export const useGlobalStreamStatus = create<GlobalStreamStatusStore>((set, get) => ({
  streams: new Map(),

  updateStream: (id, name, isConnected, error) => {
    set((state) => {
      const newStreams = new Map(state.streams);
      newStreams.set(id, {
        id,
        name,
        isConnected,
        error,
        lastUpdated: Date.now(),
      });
      return { streams: newStreams };
    });
  },

  removeStream: (id) => {
    set((state) => {
      const newStreams = new Map(state.streams);
      newStreams.delete(id);
      return { streams: newStreams };
    });
  },

  getOverallState: () => {
    const { streams } = get();
    if (streams.size === 0) return 'disconnected';

    let connected = 0;
    let hasError = false;

    streams.forEach((stream) => {
      if (stream.isConnected) connected++;
      if (stream.error) hasError = true;
    });

    if (connected === streams.size) return 'connected';
    if (connected > 0) return 'reconnecting';
    if (hasError) return 'disconnected';
    return 'connecting';
  },

  getConnectedCount: () => {
    const { streams } = get();
    let count = 0;
    streams.forEach((stream) => {
      if (stream.isConnected) count++;
    });
    return count;
  },

  getTotalCount: () => {
    return get().streams.size;
  },

  hasErrors: () => {
    const { streams } = get();
    for (const stream of streams.values()) {
      if (stream.error) return true;
    }
    return false;
  },

  getFirstError: () => {
    const { streams } = get();
    for (const stream of streams.values()) {
      if (stream.error) return stream.error;
    }
    return null;
  },
}));

/**
 * Hook to get simplified global stream status for UI consumption
 */
export function useGlobalConnectionStatus() {
  const state = useGlobalStreamStatus((s) => s.getOverallState());
  const connectedCount = useGlobalStreamStatus((s) => s.getConnectedCount());
  const totalCount = useGlobalStreamStatus((s) => s.getTotalCount());
  const hasErrors = useGlobalStreamStatus((s) => s.hasErrors());
  const firstError = useGlobalStreamStatus((s) => s.getFirstError());

  return {
    state,
    connectedCount,
    totalCount,
    hasErrors,
    error: firstError,
    isHealthy: state === 'connected' && !hasErrors,
    isEmpty: totalCount === 0,
  };
}

/**
 * Hook to register a stream with the global status tracker
 * Call this in your stream hooks to contribute to global status
 */
export function useRegisterStream(
  id: string,
  name: string,
  isConnected: boolean,
  error: string | null
) {
  const updateStream = useGlobalStreamStatus((s) => s.updateStream);
  const removeStream = useGlobalStreamStatus((s) => s.removeStream);

  // Update on mount and when status changes
  updateStream(id, name, isConnected, error);

  // Return cleanup function
  return () => removeStream(id);
}

export default useGlobalStreamStatus;
