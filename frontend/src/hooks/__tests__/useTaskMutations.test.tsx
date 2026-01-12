import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, Mock } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BrowserRouter } from 'react-router-dom';
import { ReactNode } from 'react';
import { useTaskMutations } from '../useTaskMutations';
import { tasksApi } from '@/lib/api';
import type { Task, CreateTask } from 'shared/types';

// Mock the API module
vi.mock('@/lib/api', () => ({
  tasksApi: {
    create: vi.fn(),
    createAndStart: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
  },
  attemptsApi: {
    getAll: vi.fn(),
  },
}));

// Mock analytics tracking
vi.mock('@/lib/track-analytics', () => ({
  trackTaskCreated: vi.fn(),
  trackTaskCompleted: vi.fn(),
  checkAndTrackFirstSuccess: vi.fn(),
}));

// Mock navigation hook
const mockNavigate = vi.fn();
vi.mock('@/hooks', () => ({
  useNavigateWithSearch: () => mockNavigate,
}));

// Helper to create wrapper with QueryClient
const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>{children}</BrowserRouter>
      </QueryClientProvider>
    );
  };
};

// Mock task data
const mockTask: Task = {
  id: 'task-123',
  project_id: 'project-1',
  title: 'Test Task',
  description: 'Test description',
  status: 'todo',
  parent_task_attempt: null,
  dev_server_id: null,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
};

describe('useTaskMutations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('createTask', () => {
    it('should call tasksApi.create with correct data', async () => {
      (tasksApi.create as Mock).mockResolvedValue(mockTask);

      const { result } = renderHook(() => useTaskMutations('project-1'), {
        wrapper: createWrapper(),
      });

      const createData: CreateTask = {
        project_id: 'project-1',
        title: 'New Task',
        description: 'New task description',
      };

      await act(async () => {
        result.current.createTask.mutate(createData);
      });

      await waitFor(() => {
        expect(result.current.createTask.isSuccess).toBe(true);
      });

      expect(tasksApi.create).toHaveBeenCalledWith(createData);
    });

    it('should navigate to task page on success', async () => {
      (tasksApi.create as Mock).mockResolvedValue(mockTask);

      const { result } = renderHook(() => useTaskMutations('project-1'), {
        wrapper: createWrapper(),
      });

      await act(async () => {
        result.current.createTask.mutate({
          project_id: 'project-1',
          title: 'New Task',
        });
      });

      await waitFor(() => {
        expect(result.current.createTask.isSuccess).toBe(true);
      });

      expect(mockNavigate).toHaveBeenCalledWith(
        '/projects/project-1/tasks/task-123/attempts/latest'
      );
    });

    it('should handle create error', async () => {
      const error = new Error('Failed to create task');
      (tasksApi.create as Mock).mockRejectedValue(error);

      const { result } = renderHook(() => useTaskMutations('project-1'), {
        wrapper: createWrapper(),
      });

      await act(async () => {
        result.current.createTask.mutate({
          project_id: 'project-1',
          title: 'New Task',
        });
      });

      await waitFor(() => {
        expect(result.current.createTask.isError).toBe(true);
      });

      expect(result.current.createTask.error).toBe(error);
    });
  });

  describe('updateTask', () => {
    it('should call tasksApi.update with correct data', async () => {
      const updatedTask = { ...mockTask, title: 'Updated Title' };
      (tasksApi.update as Mock).mockResolvedValue(updatedTask);

      const { result } = renderHook(() => useTaskMutations('project-1'), {
        wrapper: createWrapper(),
      });

      await act(async () => {
        result.current.updateTask.mutate({
          taskId: 'task-123',
          data: { title: 'Updated Title' },
        });
      });

      await waitFor(() => {
        expect(result.current.updateTask.isSuccess).toBe(true);
      });

      expect(tasksApi.update).toHaveBeenCalledWith('task-123', {
        title: 'Updated Title',
      });
    });
  });

  describe('deleteTask', () => {
    it('should call tasksApi.delete with task id', async () => {
      (tasksApi.delete as Mock).mockResolvedValue(undefined);

      const { result } = renderHook(() => useTaskMutations('project-1'), {
        wrapper: createWrapper(),
      });

      await act(async () => {
        result.current.deleteTask.mutate('task-123');
      });

      await waitFor(() => {
        expect(result.current.deleteTask.isSuccess).toBe(true);
      });

      expect(tasksApi.delete).toHaveBeenCalledWith('task-123');
    });

    it('should handle delete error', async () => {
      const error = new Error('Failed to delete task');
      (tasksApi.delete as Mock).mockRejectedValue(error);

      const { result } = renderHook(() => useTaskMutations('project-1'), {
        wrapper: createWrapper(),
      });

      await act(async () => {
        result.current.deleteTask.mutate('task-123');
      });

      await waitFor(() => {
        expect(result.current.deleteTask.isError).toBe(true);
      });
    });
  });

  describe('without projectId', () => {
    it('should not navigate when projectId is undefined', async () => {
      (tasksApi.create as Mock).mockResolvedValue(mockTask);

      const { result } = renderHook(() => useTaskMutations(), {
        wrapper: createWrapper(),
      });

      await act(async () => {
        result.current.createTask.mutate({
          project_id: 'project-1',
          title: 'New Task',
        });
      });

      await waitFor(() => {
        expect(result.current.createTask.isSuccess).toBe(true);
      });

      // Should not navigate when projectId is undefined
      expect(mockNavigate).not.toHaveBeenCalled();
    });
  });
});
