import { renderHook } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { useFilteredTasks } from '../useFilteredTasks';
import type { Task, TaskStatus } from 'shared/types';

// Helper to create mock tasks
const createMockTask = (overrides: Partial<Task> = {}): Task => ({
  id: `task-${Math.random().toString(36).slice(2)}`,
  project_id: 'project-1',
  title: 'Test Task',
  description: 'Test description',
  status: 'todo',
  parent_task_attempt: null,
  dev_server_id: null,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
  ...overrides,
});

describe('useFilteredTasks', () => {
  it('should return empty array when tasks is empty', () => {
    const { result } = renderHook(() => useFilteredTasks([], 'todo'));
    expect(result.current).toEqual([]);
  });

  it('should filter tasks by status', () => {
    const tasks: Task[] = [
      createMockTask({ id: '1', status: 'todo' }),
      createMockTask({ id: '2', status: 'inprogress' }),
      createMockTask({ id: '3', status: 'todo' }),
      createMockTask({ id: '4', status: 'done' }),
    ];

    const { result } = renderHook(() => useFilteredTasks(tasks, 'todo'));

    expect(result.current).toHaveLength(2);
    expect(result.current.map((t) => t.id)).toEqual(['1', '3']);
  });

  it('should filter out agent status tasks', () => {
    const tasks: Task[] = [
      createMockTask({ id: '1', status: 'agent' }),
      createMockTask({ id: '2', status: 'todo' }),
    ];

    const { result } = renderHook(() => useFilteredTasks(tasks, 'agent'));

    // Agent tasks should be filtered out even when filtering for 'agent' status
    expect(result.current).toHaveLength(0);
  });

  it('should return all tasks matching the status when no agent tasks', () => {
    const tasks: Task[] = [
      createMockTask({ id: '1', status: 'inprogress' }),
      createMockTask({ id: '2', status: 'inprogress' }),
      createMockTask({ id: '3', status: 'inprogress' }),
    ];

    const { result } = renderHook(() => useFilteredTasks(tasks, 'inprogress'));

    expect(result.current).toHaveLength(3);
  });

  it('should update when tasks change', () => {
    const initialTasks: Task[] = [createMockTask({ id: '1', status: 'todo' })];

    const { result, rerender } = renderHook(
      ({ tasks, status }) => useFilteredTasks(tasks, status),
      { initialProps: { tasks: initialTasks, status: 'todo' as TaskStatus } }
    );

    expect(result.current).toHaveLength(1);

    const updatedTasks: Task[] = [
      ...initialTasks,
      createMockTask({ id: '2', status: 'todo' }),
    ];

    rerender({ tasks: updatedTasks, status: 'todo' });

    expect(result.current).toHaveLength(2);
  });

  it('should update when status filter changes', () => {
    const tasks: Task[] = [
      createMockTask({ id: '1', status: 'todo' }),
      createMockTask({ id: '2', status: 'inprogress' }),
    ];

    const { result, rerender } = renderHook(
      ({ tasks, status }) => useFilteredTasks(tasks, status),
      { initialProps: { tasks, status: 'todo' as TaskStatus } }
    );

    expect(result.current).toHaveLength(1);
    expect(result.current[0].id).toBe('1');

    rerender({ tasks, status: 'inprogress' });

    expect(result.current).toHaveLength(1);
    expect(result.current[0].id).toBe('2');
  });
});
