import { useEffect, useCallback, useRef } from 'react';
import { CollaborationEvent } from 'shared/types';
import { useCollaboration } from '@/components/context/CollaborationProvider';
import { tasksApi, TaskWithUsersAndAttemptStatus } from '@/lib/api';

export interface RealtimeTaskSyncOptions {
  projectId: string;
  tasks: TaskWithUsersAndAttemptStatus[];
  onTasksUpdate: (tasks: TaskWithUsersAndAttemptStatus[]) => void;
  onOptimisticUpdate?: (tasks: TaskWithUsersAndAttemptStatus[]) => void;
  onSyncError?: (error: string) => void;
}

export function useRealtimeTaskSync({
  projectId,
  tasks,
  onTasksUpdate,
  onOptimisticUpdate,
  onSyncError,
}: RealtimeTaskSyncOptions) {
  const { subscribeToEvents } = useCollaboration();
  const lastSyncRef = useRef<number>(Date.now());
  const syncTimeoutRef = useRef<number | null>(null);
  const isSyncingRef = useRef(false);

  // Debounced sync function to avoid excessive API calls
  const scheduleSync = useCallback(() => {
    if (syncTimeoutRef.current) {
      clearTimeout(syncTimeoutRef.current);
    }

    syncTimeoutRef.current = setTimeout(async () => {
      if (isSyncingRef.current) return;

      try {
        isSyncingRef.current = true;
        const updatedTasks = await tasksApi.getAll(projectId);
        onTasksUpdate(updatedTasks);
        lastSyncRef.current = Date.now();
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Failed to sync tasks';
        console.error('[RealtimeTaskSync] Sync failed:', errorMessage);
        onSyncError?.(errorMessage);
      } finally {
        isSyncingRef.current = false;
      }
    }, 1000); // 1 second debounce
  }, [projectId, onTasksUpdate, onSyncError]);

  // Handle individual task updates optimistically
  const handleTaskUpdate = useCallback((event: CollaborationEvent) => {
    if (!event.data.task) return;

    const updatedTask = event.data.task;
    const updatedTasks = tasks.map(task => {
      if (task.id === updatedTask.id) {
        // Merge the updated task data with existing task data
        // This handles cases where the event data might not contain all fields
        return {
          ...task,
          ...updatedTask,
          // Preserve user information if not provided in event
          creator_username: updatedTask.creator_username || task.creator_username,
          creator_display_name: updatedTask.creator_display_name || task.creator_display_name,
          assignee_username: updatedTask.assignee_username || task.assignee_username,
          assignee_display_name: updatedTask.assignee_display_name || task.assignee_display_name,
        };
      }
      return task;
    });

    // Apply optimistic update immediately
    onOptimisticUpdate?.(updatedTasks);

    // Schedule a full sync to ensure consistency
    scheduleSync();
  }, [tasks, onOptimisticUpdate, scheduleSync]);

  // Handle task creation
  const handleTaskCreation = useCallback((event: CollaborationEvent) => {
    if (!event.data.task) return;

    const newTask = event.data.task;
    
    // Check if task already exists (avoid duplicates)
    const taskExists = tasks.some(task => task.id === newTask.id);
    if (taskExists) return;

    // Add the new task optimistically
    const updatedTasks = [...tasks, newTask];
    onOptimisticUpdate?.(updatedTasks);

    // Schedule a full sync to ensure consistency
    scheduleSync();
  }, [tasks, onOptimisticUpdate, scheduleSync]);

  // Handle task assignment changes
  const handleTaskAssignment = useCallback((event: CollaborationEvent) => {
    if (!event.data.task) return;

    const taskId = event.data.task.id;
    const assignedTo = event.data.assigned_to;
    const assignedToId = assignedTo?.id || null;

    const updatedTasks = tasks.map(task => {
      if (task.id === taskId) {
        return {
          ...task,
          assigned_to: assignedToId,
          assignee_username: assignedTo?.username || null,
          assignee_display_name: assignedTo?.display_name || null,
        };
      }
      return task;
    });

    // Apply optimistic update immediately
    onOptimisticUpdate?.(updatedTasks);

    // Schedule a full sync to ensure consistency
    scheduleSync();
  }, [tasks, onOptimisticUpdate, scheduleSync]);

  // Handle task attempt updates (affects status indicators)
  const handleTaskAttemptUpdate = useCallback((event: CollaborationEvent) => {
    if (!event.data.task) return;

    const taskId = event.data.task.id;

    const updatedTasks = tasks.map(task => {
      if (task.id === taskId) {
        // Update attempt-related flags based on event type
        if (event.event_type === 'task_attempt_created') {
          return {
            ...task,
            has_in_progress_attempt: true,
            last_attempt_failed: false,
          };
        } else if (event.event_type === 'task_attempt_approved') {
          return {
            ...task,
            has_merged_attempt: true,
            has_in_progress_attempt: false,
            last_attempt_failed: false,
          };
        }
      }
      return task;
    });

    // Apply optimistic update immediately
    onOptimisticUpdate?.(updatedTasks);

    // Schedule a full sync to ensure consistency
    scheduleSync();
  }, [tasks, onOptimisticUpdate, scheduleSync]);

  // Subscribe to collaboration events
  useEffect(() => {
    const unsubscribe = subscribeToEvents((event) => {
      // Only handle events for the current project
      if (event.project_id !== projectId) return;

      switch (event.event_type) {
        case 'task_created':
          handleTaskCreation(event);
          break;
        case 'task_updated':
          handleTaskUpdate(event);
          break;
        case 'task_assigned':
          handleTaskAssignment(event);
          break;
        case 'task_attempt_created':
        case 'task_attempt_approved':
          handleTaskAttemptUpdate(event);
          break;
        default:
          // For other events, just schedule a sync
          scheduleSync();
          break;
      }
    });

    return unsubscribe;
  }, [
    subscribeToEvents,
    projectId,
    handleTaskCreation,
    handleTaskUpdate,
    handleTaskAssignment,
    handleTaskAttemptUpdate,
    scheduleSync,
  ]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (syncTimeoutRef.current) {
        clearTimeout(syncTimeoutRef.current);
      }
    };
  }, []);

  // Manual sync function for external use
  const manualSync = useCallback(async () => {
    scheduleSync();
  }, [scheduleSync]);

  // Check if sync is needed based on time
  const isSyncStale = useCallback(() => {
    const now = Date.now();
    const timeSinceLastSync = now - lastSyncRef.current;
    return timeSinceLastSync > 30000; // 30 seconds
  }, []);

  return {
    manualSync,
    isSyncStale,
    isActive: true,
  };
}