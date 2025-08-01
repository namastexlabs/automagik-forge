import React, { useEffect, useState } from 'react';
import { CollaborationEvent } from 'shared/types';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { UserAvatar } from '@/components/ui/user-avatar';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { 
  Activity, 
  Plus, 
  Edit, 
  UserCheck, 
  CheckCircle, 
  Clock,
  User
} from 'lucide-react';

interface ActivityIndicatorProps {
  events: CollaborationEvent[];
  maxEvents?: number;
  className?: string;
}

export function ActivityIndicator({ 
  events, 
  maxEvents = 10, 
  className = '' 
}: ActivityIndicatorProps) {
  const recentEvents = events.slice(0, maxEvents);

  if (recentEvents.length === 0) {
    return (
      <Card className={className}>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center space-x-2 text-sm">
            <Activity className="h-4 w-4" />
            <span>Recent Activity</span>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">No recent activity</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className={className}>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center space-x-2 text-sm">
          <Activity className="h-4 w-4" />
          <span>Recent Activity</span>
          <Badge variant="secondary" className="ml-auto">
            {recentEvents.length}
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {recentEvents.map((event, index) => (
          <ActivityEventItem key={`${event.event_id}-${index}`} event={event} />
        ))}
      </CardContent>
    </Card>
  );
}

interface ActivityEventItemProps {
  event: CollaborationEvent;
}

function ActivityEventItem({ event }: ActivityEventItemProps) {
  const [timeAgo, setTimeAgo] = useState('');

  useEffect(() => {
    const updateTimeAgo = () => {
      const eventTime = new Date(event.timestamp);
      const now = new Date();
      const diffMs = now.getTime() - eventTime.getTime();
      const diffMinutes = Math.floor(diffMs / 60000);
      
      if (diffMinutes < 1) {
        setTimeAgo('just now');
      } else if (diffMinutes < 60) {
        setTimeAgo(`${diffMinutes}m ago`);
      } else if (diffMinutes < 1440) {
        const diffHours = Math.floor(diffMinutes / 60);
        setTimeAgo(`${diffHours}h ago`);
      } else {
        const diffDays = Math.floor(diffMinutes / 1440);
        setTimeAgo(`${diffDays}d ago`);
      }
    };

    updateTimeAgo();
    const interval = setInterval(updateTimeAgo, 60000); // Update every minute

    return () => clearInterval(interval);
  }, [event.timestamp]);

  const getEventInfo = (eventType: string) => {
    switch (eventType) {
      case 'task_created':
        return { 
          icon: Plus, 
          color: 'text-green-600 dark:text-green-400',
          bgColor: 'bg-green-100 dark:bg-green-900/20',
          label: 'created a task'
        };
      case 'task_updated':
        return { 
          icon: Edit, 
          color: 'text-blue-600 dark:text-blue-400',
          bgColor: 'bg-blue-100 dark:bg-blue-900/20',
          label: 'updated a task'
        };
      case 'task_assigned':
        return { 
          icon: UserCheck, 
          color: 'text-purple-600 dark:text-purple-400',
          bgColor: 'bg-purple-100 dark:bg-purple-900/20',
          label: 'assigned a task'
        };
      case 'task_attempt_created':
        return { 
          icon: Clock, 
          color: 'text-yellow-600 dark:text-yellow-400',
          bgColor: 'bg-yellow-100 dark:bg-yellow-900/20',
          label: 'started working on a task'
        };
      case 'task_attempt_approved':
        return { 
          icon: CheckCircle, 
          color: 'text-emerald-600 dark:text-emerald-400',
          bgColor: 'bg-emerald-100 dark:bg-emerald-900/20',
          label: 'completed a task'
        };
      case 'user_presence_updated':
        return { 
          icon: User, 
          color: 'text-gray-600 dark:text-gray-400',
          bgColor: 'bg-gray-100 dark:bg-gray-900/20',
          label: 'updated their status'
        };
      default:
        return { 
          icon: Activity, 
          color: 'text-gray-600 dark:text-gray-400',
          bgColor: 'bg-gray-100 dark:bg-gray-900/20',
          label: 'performed an action'
        };
    }
  };

  const eventInfo = getEventInfo(event.event_type);
  const EventIcon = eventInfo.icon;

  // Extract relevant information from event data
  const getEventDetails = () => {
    if (event.event_type === 'task_created' || event.event_type === 'task_updated') {
      return event.data.task?.title || 'Unknown task';
    }
    if (event.event_type === 'task_assigned') {
      const assignedTo = event.data.assigned_to;
      const taskTitle = event.data.task?.title || 'Unknown task';
      return assignedTo ? `${taskTitle} to ${assignedTo.display_name || assignedTo.username}` : taskTitle;
    }
    if (event.event_type === 'task_attempt_created' || event.event_type === 'task_attempt_approved') {
      return event.data.task?.title || 'Unknown task';
    }
    return '';
  };

  const eventDetails = getEventDetails();

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="flex items-start space-x-3 p-2 rounded-lg hover:bg-muted/50 transition-colors group">
          <div className={`p-1.5 rounded-full ${eventInfo.bgColor} flex-shrink-0`}>
            <EventIcon className={`h-3 w-3 ${eventInfo.color}`} />
          </div>
          
          <div className="flex-1 min-w-0">
            <div className="flex items-center space-x-2">
              <UserAvatar
                src={event.user_info.avatar_url}
                alt={event.user_info.display_name || event.user_info.username}
                size="sm"
                className="w-4 h-4"
              />
              <span className="text-sm font-medium truncate">
                {event.user_info.display_name || event.user_info.username}
              </span>
            </div>
            
            <p className="text-xs text-muted-foreground mt-0.5">
              {eventInfo.label}
              {eventDetails && (
                <span className="font-medium text-foreground ml-1">
                  {eventDetails}
                </span>
              )}
            </p>
          </div>
          
          <div className="flex-shrink-0 text-xs text-muted-foreground">
            {timeAgo}
          </div>
        </div>
      </TooltipTrigger>
      <TooltipContent>
        <div className="space-y-1">
          <p className="font-medium">
            {event.user_info.display_name || event.user_info.username}
          </p>
          <p className="text-xs">Action: {eventInfo.label}</p>
          {eventDetails && (
            <p className="text-xs">Target: {eventDetails}</p>
          )}
          <p className="text-xs">Time: {new Date(event.timestamp).toLocaleString()}</p>
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

interface LiveActivityBadgeProps {
  hasRecentActivity: boolean;
  activityCount?: number;
  className?: string;
}

export function LiveActivityBadge({ 
  hasRecentActivity, 
  activityCount = 0, 
  className = '' 
}: LiveActivityBadgeProps) {
  if (!hasRecentActivity || activityCount === 0) {
    return null;
  }

  return (
    <Badge variant="secondary" className={`${className} flex items-center space-x-1 animate-pulse`}>
      <Activity className="h-3 w-3" />
      <span className="text-xs">{activityCount} recent</span>
    </Badge>
  );
}

interface RecentChangeIndicatorProps {
  lastEvent: CollaborationEvent | null;
  className?: string;
}

export function RecentChangeIndicator({ 
  lastEvent, 
  className = '' 
}: RecentChangeIndicatorProps) {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    if (lastEvent) {
      setIsVisible(true);
      
      // Hide the indicator after 5 seconds
      const timeout = setTimeout(() => {
        setIsVisible(false);
      }, 5000);

      return () => clearTimeout(timeout);
    }
  }, [lastEvent]);

  if (!lastEvent || !isVisible) {
    return null;
  }

  const eventTime = new Date(lastEvent.timestamp);
  const now = new Date();
  const diffMs = now.getTime() - eventTime.getTime();
  
  // Only show for very recent events (last 30 seconds)
  if (diffMs > 30000) {
    return null;
  }

  return (
    <Badge 
      variant="outline" 
      className={`${className} flex items-center space-x-1 animate-in fade-in-50 slide-in-from-top-1 duration-300`}
    >
      <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
      <span className="text-xs">
        {lastEvent.user_info.display_name || lastEvent.user_info.username} just made changes
      </span>
    </Badge>
  );
}