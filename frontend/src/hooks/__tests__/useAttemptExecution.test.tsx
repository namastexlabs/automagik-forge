import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, Mock } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactNode } from 'react';
import { useAttemptExecution } from '../useAttemptExecution';
import { attemptsApi, executionProcessesApi } from '@/lib/api';
import type { ExecutionProcess } from 'shared/types';

// Mock the API module
vi.mock('@/lib/api', () => ({
  attemptsApi: {
    stop: vi.fn(),
  },
  executionProcessesApi: {
    getDetails: vi.fn(),
  },
}));

// Mock the store
const mockSetIsStopping = vi.fn();
vi.mock('@/stores/useTaskDetailsUiStore', () => ({
  useTaskStopping: () => ({
    isStopping: false,
    setIsStopping: mockSetIsStopping,
  }),
}));

// Mock execution processes context
const mockContextValue = {
  executionProcessesVisible: [] as ExecutionProcess[],
  isAttemptRunningVisible: false,
  isLoading: false,
};

vi.mock('@/contexts/ExecutionProcessesContext', () => ({
  useExecutionProcessesContext: () => mockContextValue,
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
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
};

// Mock execution process data
const createMockProcess = (
  overrides: Partial<ExecutionProcess> = {}
): ExecutionProcess => ({
  id: `process-${Math.random().toString(36).slice(2)}`,
  task_attempt_id: 'attempt-123',
  status: 'running',
  run_reason: 'codingagent',
  variant: null,
  pid: 12345,
  prompt: 'Test prompt',
  exit_code: null,
  dropped: false,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
  ...overrides,
});

describe('useAttemptExecution', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset mock context to default values
    mockContextValue.executionProcessesVisible = [];
    mockContextValue.isAttemptRunningVisible = false;
    mockContextValue.isLoading = false;
  });

  describe('initial state', () => {
    it('should return empty processes when context has none', () => {
      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      expect(result.current.processes).toEqual([]);
      expect(result.current.attemptData.processes).toEqual([]);
      expect(result.current.isAttemptRunning).toBe(false);
    });

    it('should return loading state from context', () => {
      mockContextValue.isLoading = true;

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      expect(result.current.isLoading).toBe(true);
    });
  });

  describe('with execution processes', () => {
    it('should return processes from context', () => {
      const mockProcesses = [
        createMockProcess({ id: 'process-1' }),
        createMockProcess({ id: 'process-2' }),
      ];
      mockContextValue.executionProcessesVisible = mockProcesses;
      mockContextValue.isAttemptRunningVisible = true;

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      expect(result.current.processes).toHaveLength(2);
      expect(result.current.isAttemptRunning).toBe(true);
    });

    it('should fetch details for setup script processes', async () => {
      const setupProcess = createMockProcess({
        id: 'setup-1',
        run_reason: 'setupscript',
      });
      const detailedProcess = { ...setupProcess, prompt: 'Detailed prompt' };

      mockContextValue.executionProcessesVisible = [setupProcess];
      (executionProcessesApi.getDetails as Mock).mockResolvedValue(
        detailedProcess
      );

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      // Wait for the query to complete
      await waitFor(() => {
        expect(executionProcessesApi.getDetails).toHaveBeenCalledWith(
          'setup-1'
        );
      });
    });
  });

  describe('stopExecution', () => {
    it('should call attemptsApi.stop when attempt is running', async () => {
      mockContextValue.isAttemptRunningVisible = true;
      (attemptsApi.stop as Mock).mockResolvedValue(undefined);

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      await act(async () => {
        await result.current.stopExecution();
      });

      expect(attemptsApi.stop).toHaveBeenCalledWith('attempt-123');
      expect(mockSetIsStopping).toHaveBeenCalledWith(true);
      expect(mockSetIsStopping).toHaveBeenCalledWith(false);
    });

    it('should not call stop when attempt is not running', async () => {
      mockContextValue.isAttemptRunningVisible = false;

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      await act(async () => {
        await result.current.stopExecution();
      });

      expect(attemptsApi.stop).not.toHaveBeenCalled();
    });

    it('should not call stop when attemptId is undefined', async () => {
      mockContextValue.isAttemptRunningVisible = true;

      const { result } = renderHook(
        () => useAttemptExecution(undefined, 'task-123'),
        { wrapper: createWrapper() }
      );

      await act(async () => {
        await result.current.stopExecution();
      });

      expect(attemptsApi.stop).not.toHaveBeenCalled();
    });

    it('should handle stop error', async () => {
      mockContextValue.isAttemptRunningVisible = true;
      const error = new Error('Failed to stop');
      (attemptsApi.stop as Mock).mockRejectedValue(error);

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      await expect(
        act(async () => {
          await result.current.stopExecution();
        })
      ).rejects.toThrow('Failed to stop');

      // Should still set isStopping back to false on error
      expect(mockSetIsStopping).toHaveBeenLastCalledWith(false);
    });
  });

  describe('attemptData', () => {
    it('should build attemptData with processes and empty details when no setup processes', () => {
      const mockProcesses = [
        createMockProcess({ run_reason: 'codingagent' }),
      ];
      mockContextValue.executionProcessesVisible = mockProcesses;

      const { result } = renderHook(
        () => useAttemptExecution('attempt-123', 'task-123'),
        { wrapper: createWrapper() }
      );

      expect(result.current.attemptData.processes).toEqual(mockProcesses);
      expect(result.current.attemptData.runningProcessDetails).toEqual({});
    });
  });
});
