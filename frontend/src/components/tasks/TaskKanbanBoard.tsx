import { memo, useEffect, useMemo, useState } from 'react';
import {
  type DragEndEvent,
  KanbanBoard,
  KanbanCards,
  KanbanHeader,
  KanbanProvider,
} from '@/components/ui/shadcn-io/kanban';
import { TaskCard } from './TaskCard';
import type { TaskStatus } from 'shared/types';
import { TaskWithUsersAndAttemptStatus } from '@/lib/api';
import { useNavigate, useParams } from 'react-router-dom';
import {
  useKeyboardShortcuts,
  useKanbanKeyboardNavigation,
} from '@/lib/keyboard-shortcuts.ts';
import { useRealtimeTaskSync } from '@/hooks/useRealtimeTaskSync';
import { useCollaboration } from '@/components/context/CollaborationProvider';
import { Badge } from '@/components/ui/badge';
import { OnlineUserBadge, ConnectionStatus } from '@/components/collaboration/UserPresence';
import { RecentChangeIndicator } from '@/components/collaboration/ActivityIndicator';
import { Activity, Users, Wifi } from 'lucide-react';

type Task = TaskWithUsersAndAttemptStatus;

interface TaskKanbanBoardProps {
  tasks: Task[];
  searchQuery?: string;
  onDragEnd: (event: DragEndEvent) => void;
  onEditTask: (task: Task) => void;
  onDeleteTask: (taskId: string) => void;
  onViewTaskDetails: (task: Task) => void;
  onTasksUpdate: (tasks: Task[]) => void;
  onOptimisticUpdate?: (tasks: Task[]) => void;
  isPanelOpen: boolean;
}

const allTaskStatuses: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const statusLabels: Record<TaskStatus, string> = {
  todo: 'To Do',
  inprogress: 'In Progress',
  inreview: 'In Review',
  done: 'Done',
  cancelled: 'Cancelled',
};

const statusBoardColors: Record<TaskStatus, string> = {
  todo: 'hsl(var(--neutral))',
  inprogress: 'hsl(var(--info))',
  inreview: 'hsl(var(--warning))',
  done: 'hsl(var(--success))',
  cancelled: 'hsl(var(--destructive))',
};

function TaskKanbanBoard({
  tasks,
  searchQuery = '',
  onDragEnd,
  onEditTask,
  onDeleteTask,
  onViewTaskDetails,
  onTasksUpdate,
  onOptimisticUpdate,
  isPanelOpen,
}: TaskKanbanBoardProps) {
  const { projectId, taskId } = useParams<{
    projectId: string;
    taskId?: string;
  }>();
  const navigate = useNavigate();
  
  // Real-time collaboration state
  const { 
    isConnected, 
    isConnecting, 
    connectionError, 
    isOnline,
    onlineUsers, 
    lastEvent,
    retry
  } = useCollaboration();
  
  const [syncError, setSyncError] = useState<string | null>(null);

  useKeyboardShortcuts({
    navigate,
    currentPath: `/projects/${projectId}/tasks${taskId ? `/${taskId}` : ''}`,
  });

  const [focusedTaskId, setFocusedTaskId] = useState<string | null>(
    taskId || null
  );
  const [focusedStatus, setFocusedStatus] = useState<TaskStatus | null>(null);

  // Real-time task synchronization
  useRealtimeTaskSync({
    projectId: projectId!,
    tasks,
    onTasksUpdate,
    onOptimisticUpdate,
    onSyncError: setSyncError,
  });

  // Memoize filtered tasks
  const filteredTasks = useMemo(() => {
    if (!searchQuery.trim()) {
      return tasks;
    }
    const query = searchQuery.toLowerCase();
    return tasks.filter(
      (task) =>
        task.title.toLowerCase().includes(query) ||
        (task.description && task.description.toLowerCase().includes(query))
    );
  }, [tasks, searchQuery]);

  // Memoize grouped tasks
  const groupedTasks = useMemo(() => {
    const groups: Record<TaskStatus, Task[]> = {} as Record<TaskStatus, Task[]>;
    allTaskStatuses.forEach((status) => {
      groups[status] = [];
    });
    filteredTasks.forEach((task) => {
      const normalizedStatus = task.status.toLowerCase() as TaskStatus;
      if (groups[normalizedStatus]) {
        groups[normalizedStatus].push(task);
      } else {
        groups['todo'].push(task);
      }
    });
    return groups;
  }, [filteredTasks]);

  // Sync focus state with taskId param
  useEffect(() => {
    if (taskId) {
      const found = filteredTasks.find((t) => t.id === taskId);
      if (found) {
        setFocusedTaskId(taskId);
        setFocusedStatus((found.status.toLowerCase() as TaskStatus) || null);
      }
    }
  }, [taskId, filteredTasks]);

  // If no taskId in params, keep last focused, or focus first available
  useEffect(() => {
    if (!taskId && !focusedTaskId) {
      for (const status of allTaskStatuses) {
        if (groupedTasks[status] && groupedTasks[status].length > 0) {
          setFocusedTaskId(groupedTasks[status][0].id);
          setFocusedStatus(status);
          break;
        }
      }
    }
  }, [taskId, focusedTaskId, groupedTasks]);

  // Keyboard navigation handler
  useKanbanKeyboardNavigation({
    focusedTaskId,
    setFocusedTaskId: (id) => {
      setFocusedTaskId(id as string | null);
      if (isPanelOpen) {
        const task = filteredTasks.find((t: any) => t.id === id);
        if (task) {
          onViewTaskDetails(task);
        }
      }
    },
    focusedStatus,
    setFocusedStatus: (status) => setFocusedStatus(status as TaskStatus | null),
    groupedTasks,
    filteredTasks,
    allTaskStatuses,
  });

  return (
    <div className="space-y-4">
      {/* Collaboration Status Bar */}
      <div className="flex items-center justify-between bg-muted/30 rounded-lg p-3">
        <div className="flex items-center space-x-4">
          <ConnectionStatus 
            isConnected={isConnected}
            isConnecting={isConnecting}
            error={connectionError}
            isOnline={isOnline}
            onRetry={retry}
          />
          <OnlineUserBadge onlineCount={onlineUsers.length} />
          {onlineUsers.length > 0 && (
            <div className="flex items-center space-x-1 text-xs text-muted-foreground">
              <Users className="h-3 w-3" />
              <span>
                {onlineUsers.slice(0, 3).map(user => user.display_name || user.username).join(', ')}
                {onlineUsers.length > 3 && ` +${onlineUsers.length - 3} more`}
              </span>
            </div>
          )}
        </div>
        <div className="flex items-center space-x-2">
          {syncError && (
            <Badge variant="destructive" className="text-xs">
              Sync Error: {syncError}
            </Badge>
          )}
          <RecentChangeIndicator lastEvent={lastEvent} />
        </div>
      </div>

      {/* Kanban Board */}
      <KanbanProvider onDragEnd={onDragEnd}>
        {Object.entries(groupedTasks).map(([status, statusTasks]) => (
          <KanbanBoard key={status} id={status as TaskStatus}>
            <KanbanHeader
              name={statusLabels[status as TaskStatus]}
              color={statusBoardColors[status as TaskStatus]}
            />
            <KanbanCards>
              {statusTasks.map((task, index) => (
                <TaskCard
                  key={task.id}
                  task={task}
                  index={index}
                  status={status}
                  onEdit={onEditTask}
                  onDelete={onDeleteTask}
                  onViewDetails={onViewTaskDetails}
                  isFocused={focusedTaskId === task.id}
                  tabIndex={focusedTaskId === task.id ? 0 : -1}
                />
              ))}
            </KanbanCards>
          </KanbanBoard>
        ))}
      </KanbanProvider>
    </div>
  );
}

export default memo(TaskKanbanBoard);
