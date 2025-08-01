import React from 'react';
import { UserPresence as UserPresenceType, PresenceStatus } from 'shared/types';
import { UserAvatar } from '@/components/ui/user-avatar';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { Users, Clock, Wifi, WifiOff } from 'lucide-react';

interface UserPresenceListProps {
  presence: UserPresenceType[];
  className?: string;
}

export function UserPresenceList({ presence, className = '' }: UserPresenceListProps) {
  const onlineUsers = presence.filter(p => p.status === PresenceStatus.Online);
  const awayUsers = presence.filter(p => p.status === PresenceStatus.Away);
  const offlineUsers = presence.filter(p => p.status === PresenceStatus.Offline);

  if (presence.length === 0) {
    return (
      <Card className={className}>
        <CardContent className="p-4">
          <div className="flex items-center space-x-2 text-muted-foreground">
            <Users className="h-4 w-4" />
            <span className="text-sm">No users connected</span>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className={className}>
      <CardContent className="p-4">
        <div className="space-y-4">
          {/* Online Users */}
          {onlineUsers.length > 0 && (
            <div>
              <div className="flex items-center space-x-2 mb-2">
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                <span className="text-sm font-medium text-green-700 dark:text-green-400">
                  Online ({onlineUsers.length})
                </span>
              </div>
              <div className="space-y-2">
                {onlineUsers.map(user => (
                  <UserPresenceItem key={user.user_id} presence={user} />
                ))}
              </div>
            </div>
          )}

          {/* Away Users */}
          {awayUsers.length > 0 && (
            <div>
              <div className="flex items-center space-x-2 mb-2">
                <div className="w-2 h-2 bg-yellow-500 rounded-full" />
                <span className="text-sm font-medium text-yellow-700 dark:text-yellow-400">
                  Away ({awayUsers.length})
                </span>
              </div>
              <div className="space-y-2">
                {awayUsers.map(user => (
                  <UserPresenceItem key={user.user_id} presence={user} />
                ))}
              </div>
            </div>
          )}

          {/* Offline Users */}
          {offlineUsers.length > 0 && (
            <div>
              <div className="flex items-center space-x-2 mb-2">
                <div className="w-2 h-2 bg-gray-400 rounded-full" />
                <span className="text-sm font-medium text-gray-600 dark:text-gray-400">
                  Offline ({offlineUsers.length})
                </span>
              </div>
              <div className="space-y-2">
                {offlineUsers.map(user => (
                  <UserPresenceItem key={user.user_id} presence={user} />
                ))}
              </div>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

interface UserPresenceItemProps {
  presence: UserPresenceType;
}

function UserPresenceItem({ presence }: UserPresenceItemProps) {
  const getStatusInfo = (status: PresenceStatus) => {
    switch (status) {
      case PresenceStatus.Online:
        return { color: 'bg-green-500', icon: Wifi, label: 'Online' };
      case PresenceStatus.Away:
        return { color: 'bg-yellow-500', icon: Clock, label: 'Away' };
      case PresenceStatus.Offline:
        return { color: 'bg-gray-400', icon: WifiOff, label: 'Offline' };
      default:
        return { color: 'bg-gray-400', icon: WifiOff, label: 'Unknown' };
    }
  };

  const statusInfo = getStatusInfo(presence.status);
  const StatusIcon = statusInfo.icon;
  const lastSeenDate = new Date(presence.last_seen);
  const now = new Date();
  const timeDiff = now.getTime() - lastSeenDate.getTime();
  const minutesAgo = Math.floor(timeDiff / (1000 * 60));

  const getTimeAgoString = () => {
    if (presence.status === PresenceStatus.Online) {
      return 'Active now';
    }
    
    if (minutesAgo < 1) {
      return 'Just now';
    } else if (minutesAgo < 60) {
      return `${minutesAgo}m ago`;
    } else if (minutesAgo < 1440) {
      const hoursAgo = Math.floor(minutesAgo / 60);
      return `${hoursAgo}h ago`;
    } else {
      const daysAgo = Math.floor(minutesAgo / 1440);
      return `${daysAgo}d ago`;
    }
  };

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="flex items-center space-x-3 p-2 rounded-lg hover:bg-muted/50 transition-colors">
          <div className="relative">
            <UserAvatar 
              src={presence.avatar_url} 
              alt={presence.display_name || presence.username}
              size="sm"
            />
            <div 
              className={`absolute -bottom-0.5 -right-0.5 w-3 h-3 ${statusInfo.color} rounded-full border-2 border-background`}
            />
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium truncate">
              {presence.display_name || presence.username}
            </p>
            <p className="text-xs text-muted-foreground truncate">
              {getTimeAgoString()}
            </p>
          </div>
          <StatusIcon className="h-3 w-3 text-muted-foreground flex-shrink-0" />
        </div>
      </TooltipTrigger>
      <TooltipContent>
        <div className="space-y-1">
          <p className="font-medium">{presence.display_name || presence.username}</p>
          <p className="text-xs">Status: {statusInfo.label}</p>
          <p className="text-xs">Last seen: {lastSeenDate.toLocaleString()}</p>
          {presence.current_project && (
            <p className="text-xs">Project: {presence.current_project}</p>
          )}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

interface OnlineUserBadgeProps {
  onlineCount: number;
  className?: string;
}

export function OnlineUserBadge({ onlineCount, className = '' }: OnlineUserBadgeProps) {
  if (onlineCount === 0) {
    return null;
  }

  return (
    <Badge variant="secondary" className={`${className} flex items-center space-x-1`}>
      <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
      <Users className="h-3 w-3" />
      <span>{onlineCount}</span>
    </Badge>
  );
}

interface ConnectionStatusProps {
  isConnected: boolean;
  isConnecting: boolean;
  error?: string | null;
  className?: string;
}

export function ConnectionStatus({ 
  isConnected, 
  isConnecting, 
  error, 
  className = '' 
}: ConnectionStatusProps) {
  if (isConnecting) {
    return (
      <Badge variant="outline" className={`${className} flex items-center space-x-1`}>
        <div className="w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
        <span className="text-xs">Connecting...</span>
      </Badge>
    );
  }

  if (error) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Badge variant="destructive" className={`${className} flex items-center space-x-1`}>
            <WifiOff className="h-3 w-3" />
            <span className="text-xs">Offline</span>
          </Badge>
        </TooltipTrigger>
        <TooltipContent>
          <p className="text-xs">Connection error: {error}</p>
        </TooltipContent>
      </Tooltip>
    );
  }

  if (isConnected) {
    return (
      <Badge variant="secondary" className={`${className} flex items-center space-x-1`}>
        <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
        <Wifi className="h-3 w-3" />
        <span className="text-xs">Live</span>
      </Badge>
    );
  }

  return (
    <Badge variant="outline" className={`${className} flex items-center space-x-1`}>
      <WifiOff className="h-3 w-3" />
      <span className="text-xs">Offline</span>
    </Badge>
  );
}