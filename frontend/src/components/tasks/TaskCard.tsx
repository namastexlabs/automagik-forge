import { KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { KanbanCard } from '@/components/ui/shadcn-io/kanban';
import { UserAvatar } from '@/components/ui/user-avatar';
import {
  CheckCircle,
  Edit,
  Loader2,
  MoreHorizontal,
  Trash2,
  User,
  XCircle,
  Activity,
  Clock,
  Wifi,
} from 'lucide-react';
import { is_planning_executor_type } from '@/lib/utils';
import { TaskWithUsersAndAttemptStatus } from '@/lib/api';
import { useCollaboration } from '@/components/context/CollaborationProvider';
import { PresenceStatus } from 'shared/types';

// Use the combined type from API
type Task = TaskWithUsersAndAttemptStatus;

interface TaskCardProps {
  task: Task;
  index: number;
  status: string;
  onEdit: (task: Task) => void;
  onDelete: (taskId: string) => void;
  onViewDetails: (task: Task) => void;
  isFocused: boolean;
  tabIndex?: number;
}

export function TaskCard({
  task,
  index,
  status,
  onEdit,
  onDelete,
  onViewDetails,
  isFocused,
  tabIndex = -1,
}: TaskCardProps) {
  const localRef = useRef<HTMLDivElement>(null);
  const { currentPresence, lastEvent } = useCollaboration();
  const [hasRecentUpdate, setHasRecentUpdate] = useState(false);
  const [lastUpdateBy, setLastUpdateBy] = useState<string | null>(null);

  useEffect(() => {
    if (isFocused && localRef.current) {
      localRef.current.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      localRef.current.focus();
    }
  }, [isFocused]);

  // Track recent updates to this specific task
  useEffect(() => {
    if (lastEvent && 
        (lastEvent.event_type === 'task_updated' || 
         lastEvent.event_type === 'task_assigned' ||
         lastEvent.event_type === 'task_attempt_created' ||
         lastEvent.event_type === 'task_attempt_approved') &&
        lastEvent.data.task?.id === task.id) {
      
      setHasRecentUpdate(true);
      setLastUpdateBy(lastEvent.user_info.display_name || lastEvent.user_info.username);

      // Clear the recent update indicator after 10 seconds
      const timeout = setTimeout(() => {
        setHasRecentUpdate(false);
        setLastUpdateBy(null);
      }, 10000);

      return () => clearTimeout(timeout);
    }
  }, [lastEvent, task.id]);

  // Get presence info for assigned user
  const assigneePresence = currentPresence.find(p => p.user_id === task.assigned_to);
  const isAssigneeOnline = assigneePresence?.status === PresenceStatus.Online;

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Backspace') {
        onDelete(task.id);
      } else if (e.key === 'Enter' || e.key === ' ') {
        onViewDetails(task);
      }
    },
    [task, onDelete, onViewDetails]
  );

  const handleClick = useCallback(() => {
    onViewDetails(task);
  }, [task, onViewDetails]);

  return (
    <KanbanCard
      key={task.id}
      id={task.id}
      name={task.title}
      index={index}
      parent={status}
      onClick={handleClick}
      tabIndex={tabIndex}
      forwardedRef={localRef}
      onKeyDown={handleKeyDown}
      className={hasRecentUpdate ? 'ring-2 ring-blue-400 ring-opacity-50' : ''}
    >
      <div className="space-y-2">
        {/* Recent update indicator */}
        {hasRecentUpdate && lastUpdateBy && (
          <div className="flex items-center space-x-2 px-2 py-1 bg-blue-50 dark:bg-blue-900/20 rounded text-xs animate-in fade-in-50 slide-in-from-top-1">
            <Activity className="h-3 w-3 text-blue-500 animate-pulse" />
            <span className="text-blue-700 dark:text-blue-300">
              Recently updated by {lastUpdateBy}
            </span>
          </div>
        )}

        <div className="flex items-start justify-between">
          <div className="flex-1 pr-2">
            <div className="mb-1">
              <h4 className="font-medium text-sm break-words">
                {task.latest_attempt_executor &&
                  is_planning_executor_type(task.latest_attempt_executor) && (
                    <Badge className="bg-blue-600 text-white hover:bg-blue-700 text-xs font-medium px-1.5 py-0.5 h-4 text-[10px] mr-1">
                      PLAN
                    </Badge>
                  )}
                {task.title}
              </h4>
            </div>
          </div>
          <div className="flex items-center space-x-1">
            {/* In Progress Spinner */}
            {task.has_in_progress_attempt && (
              <Loader2 className="h-3 w-3 animate-spin text-blue-500" />
            )}
            {/* Merged Indicator */}
            {task.has_merged_attempt && (
              <CheckCircle className="h-3 w-3 text-green-500" />
            )}
            {/* Failed Indicator */}
            {task.last_attempt_failed && !task.has_merged_attempt && (
              <XCircle className="h-3 w-3 text-red-500" />
            )}
            {/* Assignee Online Indicator */}
            {isAssigneeOnline && task.assigned_to && (
              <div className="flex items-center space-x-1">
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                <Wifi className="h-3 w-3 text-green-500" />
              </div>
            )}
            {/* Actions Menu */}
            <div
              onPointerDown={(e) => e.stopPropagation()}
              onMouseDown={(e) => e.stopPropagation()}
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
            >
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0 hover:bg-muted"
                  >
                    <MoreHorizontal className="h-3 w-3" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={() => onEdit(task)}>
                    <Edit className="h-4 w-4 mr-2" />
                    Edit
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => onDelete(task.id)}
                    className="text-destructive"
                  >
                    <Trash2 className="h-4 w-4 mr-2" />
                    Delete
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </div>
        {task.description && (
          <div>
            <p className="text-xs text-muted-foreground break-words">
              {task.description.length > 130
                ? `${task.description.substring(0, 130)}...`
                : task.description}
            </p>
          </div>
        )}
        
        {/* User Attribution */}
        <div className="flex items-center justify-between text-xs text-muted-foreground mt-2">
          <div className="flex items-center space-x-2">
            {task.creator_username && (
              <div className="flex items-center space-x-1">
                <User className="h-3 w-3" />
                <span>
                  {task.creator_display_name || task.creator_username}
                </span>
              </div>
            )}
          </div>
          {task.assignee_username && (
            <div className="flex items-center space-x-1">
              <span className="text-[10px] font-medium">ASSIGNED</span>
              <div className="flex items-center space-x-1">
                <div className="relative">
                  <UserAvatar
                    size="sm"
                    className="w-4 h-4"
                  />
                  {/* Presence indicator for assignee */}
                  {assigneePresence && (
                    <div 
                      className={`absolute -bottom-0.5 -right-0.5 w-2 h-2 rounded-full border border-background ${
                        assigneePresence.status === PresenceStatus.Online 
                          ? 'bg-green-500 animate-pulse' 
                          : assigneePresence.status === PresenceStatus.Away
                          ? 'bg-yellow-500'
                          : 'bg-gray-400'
                      }`}
                    />
                  )}
                </div>
                <span className="text-[10px]">
                  {task.assignee_display_name || task.assignee_username}
                </span>
                {/* Online indicator text */}
                {isAssigneeOnline && (
                  <span className="text-[9px] text-green-600 dark:text-green-400 font-medium">
                    ONLINE
                  </span>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </KanbanCard>
  );
}
