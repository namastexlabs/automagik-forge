import {
  TaskAttemptDataContext,
} from '@/components/context/taskDetailsContext';
import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  ExecutionProcess,
  NormalizedConversation,
  NormalizedEntry,
} from 'shared/types';

const useNormalizedConversation = ({
  executionProcess,
  onConversationUpdate,
  onDisplayEntriesChange,
  visibleEntriesNum,
}: {
  executionProcess?: ExecutionProcess;
  onConversationUpdate?: () => void;
  onDisplayEntriesChange?: (num: number) => void;
  visibleEntriesNum?: number;
}) => {
  const { attemptData } = useContext(TaskAttemptDataContext);

  // Development-only logging helper
  const debugLog = useCallback((message: string, ...args: any[]) => {
    if (import.meta.env.DEV) {
      console.log(message, ...args);
    }
  }, []);

  const [conversation, setConversation] =
    useState<NormalizedConversation | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Track fetched processes to prevent redundant database calls
  const fetchedProcesses = useRef(new Set<string>());

  // Process-based data fetching - fetch from database
  useEffect(() => {
    if (!executionProcess?.id) {
      return;
    }
    const processId = executionProcess.id;

    debugLog(`🎯 Data: Loading process ${processId}`);

    // Reset conversation state when switching processes
    if (!fetchedProcesses.current.has(processId)) {
      setConversation(null);
      setLoading(true);
      setError(null);

      // Clear fetch tracking for old processes (keep memory bounded)
      if (fetchedProcesses.current.size > 10) {
        fetchedProcesses.current.clear();
      }
    }

    // Database fetch for all processes
    debugLog(`📋 Data: Using database for process ${processId}`);
    const logs = attemptData.allLogs.find(
      (entry) => entry.id === executionProcess.id
    )?.normalized_conversation;
    
    if (logs) {
      setConversation((prev) => {
        // Only update if content actually changed - use lightweight comparison
        if (
          !prev ||
          prev.entries.length !== logs.entries.length ||
          prev.prompt !== logs.prompt
        ) {
          // Notify parent component of conversation update
          if (onConversationUpdate) {
            // Use setTimeout to ensure state update happens first
            setTimeout(onConversationUpdate, 0);
          }
          return logs;
        }
        return prev;
      });
      fetchedProcesses.current.add(processId);
    }
    setLoading(false);
  }, [
    executionProcess?.id,
    executionProcess?.status,
    attemptData.allLogs,
    debugLog,
    onConversationUpdate,
  ]);

  // Memoize display entries to avoid unnecessary re-renders
  const displayEntries = useMemo(() => {
    if (!conversation?.entries) return [];

    // Filter out any null entries that may have been created by duplicate patch application
    const displayEntries = conversation.entries.filter(
      (entry): entry is NormalizedEntry =>
        Boolean(entry && (entry as NormalizedEntry).entry_type)
    );
    onDisplayEntriesChange?.(displayEntries.length);
    if (visibleEntriesNum && displayEntries.length > visibleEntriesNum) {
      return displayEntries.slice(-visibleEntriesNum);
    }

    return displayEntries;
  }, [conversation?.entries, onDisplayEntriesChange, visibleEntriesNum]);

  return {
    displayEntries,
    conversation,
    loading,
    error,
  };
};

export default useNormalizedConversation;