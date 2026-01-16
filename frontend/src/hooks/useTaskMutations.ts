import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigateWithSearch } from '@/hooks';
import { tasksApi, attemptsApi } from '@/lib/api';
import { paths } from '@/lib/paths';
import { queryKeys } from '@/lib/queryKeys';
import {
  trackTaskCreated,
  trackTaskCompleted,
  checkAndTrackFirstSuccess,
} from '@/lib/track-analytics';
import type {
  CreateTask,
  CreateAndStartTaskRequest,
  Task,
  TaskWithAttemptStatus,
  UpdateTask,
} from 'shared/types';

// Helper to safely extract executor string from various object shapes
const getExecutorString = (obj: unknown): string => {
  if (!obj || typeof obj !== 'object') return 'unknown';
  const record = obj as Record<string, unknown>;
  if (typeof record.executor === 'string') return record.executor;
  if (record.executor_profile && typeof record.executor_profile === 'object') {
    const profile = record.executor_profile as Record<string, unknown>;
    if (typeof profile.executor === 'string') return profile.executor;
  }
  return 'unknown';
};

export function useTaskMutations(projectId?: string) {
  const queryClient = useQueryClient();
  const navigate = useNavigateWithSearch();

  const invalidateQueries = (taskId?: string) => {
    queryClient.invalidateQueries({
      queryKey: queryKeys.tasks.byProject(projectId),
    });
    if (taskId) {
      queryClient.invalidateQueries({
        queryKey: queryKeys.tasks.detail(taskId),
      });
      // Also invalidate attempts for event-driven cache sync
      queryClient.invalidateQueries({
        queryKey: queryKeys.taskAttempts.byTask(taskId),
      });
    }
  };

  const createTask = useMutation({
    mutationFn: (data: CreateTask) => tasksApi.create(data),
    onSuccess: (createdTask: Task, variables: CreateTask) => {
      // Track task creation with analytics
      // Note: executor info is not in CreateTask, will be tracked from config
      trackTaskCreated({
        executor: getExecutorString(createdTask),
        has_description: !!variables.description,
        prompt_length: variables.description?.length ?? 0,
        is_subtask: !!variables.parent_task_attempt,
      });

      invalidateQueries(createdTask.id); // Include task ID to invalidate attempts
      if (projectId) {
        navigate(`${paths.task(projectId, createdTask.id)}/attempts/latest`);
      }
    },
    onError: (err) => {
      console.error('Failed to create task:', err);
    },
  });

  const createAndStart = useMutation({
    mutationFn: (data: CreateAndStartTaskRequest) =>
      tasksApi.createAndStart(data),
    onSuccess: (
      createdTask: TaskWithAttemptStatus,
      variables: CreateAndStartTaskRequest
    ) => {
      // Track task creation with analytics
      trackTaskCreated({
        executor: getExecutorString(variables.executor_profile_id),
        has_description: !!variables.task.description,
        prompt_length: variables.task.description?.length ?? 0,
        is_subtask: !!variables.task.parent_task_attempt,
      });

      invalidateQueries(createdTask.id); // Include task ID to invalidate attempts
      if (projectId) {
        navigate(`${paths.task(projectId, createdTask.id)}/attempts/latest`);
      }
    },
    onError: (err) => {
      console.error('Failed to create and start task:', err);
    },
  });

  const updateTask = useMutation({
    mutationFn: ({ taskId, data }: { taskId: string; data: UpdateTask }) =>
      tasksApi.update(taskId, data),
    onSuccess: async (
      updatedTask: Task,
      variables: { taskId: string; data: UpdateTask }
    ) => {
      // Track task completion if status changed to 'done'
      if (variables.data.status === 'done' && updatedTask.status === 'done') {
        const durationSeconds = updatedTask.created_at
          ? Math.floor(
              (Date.now() - new Date(updatedTask.created_at).getTime()) / 1000
            )
          : 0;

        // Fetch attempts to get executor and attempt count (backend data)
        try {
          const attempts = await attemptsApi.getAll(updatedTask.id);
          const latestAttempt = attempts[0]; // Sorted newest first
          const executor = getExecutorString(latestAttempt);
          const attemptCount = attempts.length;

          trackTaskCompleted({
            executor,
            duration_seconds: durationSeconds,
            attempt_count: attemptCount,
            success: true,
            had_dev_server: !!updatedTask.dev_server_id, // Now using real backend field!
          });

          // Check if this is first success and track it
          checkAndTrackFirstSuccess(updatedTask.id, executor, attemptCount);
        } catch (err) {
          console.warn('Failed to fetch attempts for telemetry:', err);
          // Fallback to basic tracking without attempt data
          trackTaskCompleted({
            executor: 'unknown',
            duration_seconds: durationSeconds,
            attempt_count: 1,
            success: true,
            had_dev_server: !!updatedTask.dev_server_id,
          });
        }
      }

      invalidateQueries(updatedTask.id);
    },
    onError: (err) => {
      console.error('Failed to update task:', err);
    },
  });

  const deleteTask = useMutation({
    mutationFn: (taskId: string) => tasksApi.delete(taskId),
    onSuccess: (_: unknown, taskId: string) => {
      invalidateQueries(taskId);
      // Remove single-task cache entry to avoid stale data flashes
      queryClient.removeQueries({
        queryKey: queryKeys.tasks.detail(taskId),
        exact: true,
      });
    },
    onError: (err) => {
      console.error('Failed to delete task:', err);
    },
  });

  return {
    createTask,
    createAndStart,
    updateTask,
    deleteTask,
  };
}
