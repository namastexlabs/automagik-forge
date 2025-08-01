import { useEffect, useState, useCallback } from 'react';
import { CollaborationEvent } from 'shared/types';
import { useCollaboration } from '@/components/context/CollaborationProvider';
import { useAuth } from '@/components/auth-provider';
import { useToast } from '@/components/ui/toast';
import { UserAvatar } from '@/components/ui/user-avatar';
import { 
  Bell, 
  BellOff, 
  Volume2, 
  VolumeX,
  Plus, 
  Edit, 
  UserCheck, 
  CheckCircle, 
  Clock 
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';

// Sound files for different event types (using existing sound system)
const EVENT_SOUNDS = {
  task_created: '/sounds/abstract-sound1.wav',
  task_updated: '/sounds/abstract-sound2.wav',
  task_assigned: '/sounds/abstract-sound3.wav',
  task_attempt_created: '/sounds/abstract-sound4.wav',
  task_attempt_approved: '/sounds/rooster.wav',
} as const;

interface NotificationSettings {
  enabled: boolean;
  soundEnabled: boolean;
  showAssignments: boolean;
  showTaskUpdates: boolean;
  showTaskCreations: boolean;
  showCompletions: boolean;
}

const DEFAULT_SETTINGS: NotificationSettings = {
  enabled: true,
  soundEnabled: true,
  showAssignments: true,
  showTaskUpdates: true,
  showTaskCreations: true,
  showCompletions: true,
};

// Hook for managing collaboration notifications
export function useCollaborationNotifications() {
  const { subscribeToEvents } = useCollaboration();
  const { user } = useAuth();
  const { addToast } = useToast();
  const [settings, setSettings] = useState<NotificationSettings>(() => {
    const saved = localStorage.getItem('collaboration-notification-settings');
    return saved ? { ...DEFAULT_SETTINGS, ...JSON.parse(saved) } : DEFAULT_SETTINGS;
  });

  // Save settings to localStorage
  useEffect(() => {
    localStorage.setItem('collaboration-notification-settings', JSON.stringify(settings));
  }, [settings]);

  // Play notification sound
  const playNotificationSound = useCallback((eventType: string) => {
    if (!settings.soundEnabled) return;

    const soundFile = EVENT_SOUNDS[eventType as keyof typeof EVENT_SOUNDS];
    if (soundFile) {
      const audio = new Audio(soundFile);
      audio.volume = 0.3; // Keep it subtle
      audio.play().catch(console.error);
    }
  }, [settings.soundEnabled]);

  // Format notification message
  const getNotificationMessage = useCallback((event: CollaborationEvent) => {
    const userName = event.user_info.display_name || event.user_info.username;
    
    switch (event.event_type) {
      case 'task_created':
        return {
          title: 'New Task Created',
          description: `${userName} created "${event.data.task?.title || 'a new task'}"`,
          icon: Plus,
        };
      case 'task_updated':
        return {
          title: 'Task Updated',
          description: `${userName} updated "${event.data.task?.title || 'a task'}"`,
          icon: Edit,
        };
      case 'task_assigned':
        const assignedTo = event.data.assigned_to;
        const taskTitle = event.data.task?.title || 'a task';
        if (assignedTo) {
          return {
            title: 'Task Assigned',
            description: `${userName} assigned "${taskTitle}" to ${assignedTo.display_name || assignedTo.username}`,
            icon: UserCheck,
          };
        } else {
          return {
            title: 'Task Unassigned',
            description: `${userName} removed assignment from "${taskTitle}"`,
            icon: UserCheck,
          };
        }
      case 'task_attempt_created':
        return {
          title: 'Work Started',
          description: `${userName} started working on "${event.data.task?.title || 'a task'}"`,
          icon: Clock,
        };
      case 'task_attempt_approved':
        return {
          title: 'Task Completed',
          description: `${userName} completed "${event.data.task?.title || 'a task'}"`,
          icon: CheckCircle,
        };
      default:
        return {
          title: 'Team Update',
          description: `${userName} made changes`,
          icon: Bell,
        };
    }
  }, []);

  // Should show notification for this event
  const shouldShowNotification = useCallback((event: CollaborationEvent) => {
    if (!settings.enabled) return false;
    if (event.user_id === user?.id) return false; // Don't show notifications for own actions

    switch (event.event_type) {
      case 'task_created':
        return settings.showTaskCreations;
      case 'task_updated':
        return settings.showTaskUpdates;
      case 'task_assigned':
        // Always show if assigned to current user
        if (event.data.assigned_to?.id === user?.id) return true;
        return settings.showAssignments;
      case 'task_attempt_created':
        return settings.showTaskUpdates;
      case 'task_attempt_approved':
        return settings.showCompletions;
      default:
        return false;
    }
  }, [settings, user?.id]);

  // Handle collaboration events
  useEffect(() => {
    const unsubscribe = subscribeToEvents((event) => {
      if (shouldShowNotification(event)) {
        const notification = getNotificationMessage(event);
        const NotificationIcon = notification.icon;

        // Show toast notification
        addToast(
          <div className="flex items-start space-x-3">
            <UserAvatar
              src={event.user_info.avatar_url}
              alt={event.user_info.display_name || event.user_info.username}
              size="sm"
            />
            <div className="flex-1">
              <div className="flex items-center space-x-2 mb-1">
                <NotificationIcon className="h-4 w-4" />
                <span className="font-medium text-sm">{notification.title}</span>
              </div>
              <p className="text-sm text-muted-foreground">{notification.description}</p>
            </div>
          </div>,
          {
            duration: 4000,
            type: 'info',
          }
        );

        // Play sound
        playNotificationSound(event.event_type);
      }
    });

    return unsubscribe;
  }, [subscribeToEvents, shouldShowNotification, getNotificationMessage, playNotificationSound]);

  return {
    settings,
    updateSettings: setSettings,
  };
}

// Notification settings component
interface NotificationSettingsProps {
  className?: string;
}

export function NotificationSettings({ className = '' }: NotificationSettingsProps) {
  const { settings, updateSettings } = useCollaborationNotifications();

  const toggleSetting = (key: keyof NotificationSettings) => {
    updateSettings(prev => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <Card className={className}>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center space-x-2 text-sm">
          <Bell className="h-4 w-4" />
          <span>Notification Settings</span>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Master toggle */}
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Bell className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm font-medium">Enable Notifications</span>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => toggleSetting('enabled')}
            className={`h-6 w-12 p-0 ${
              settings.enabled 
                ? 'bg-green-100 dark:bg-green-900/20' 
                : 'bg-gray-100 dark:bg-gray-900/20'
            }`}
          >
            {settings.enabled ? (
              <Bell className="h-3 w-3 text-green-600 dark:text-green-400" />
            ) : (
              <BellOff className="h-3 w-3 text-gray-600 dark:text-gray-400" />
            )}
          </Button>
        </div>

        {/* Sound toggle */}
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Volume2 className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm font-medium">Sound Alerts</span>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => toggleSetting('soundEnabled')}
            disabled={!settings.enabled}
            className={`h-6 w-12 p-0 ${
              settings.soundEnabled && settings.enabled
                ? 'bg-blue-100 dark:bg-blue-900/20' 
                : 'bg-gray-100 dark:bg-gray-900/20'
            }`}
          >
            {settings.soundEnabled && settings.enabled ? (
              <Volume2 className="h-3 w-3 text-blue-600 dark:text-blue-400" />
            ) : (
              <VolumeX className="h-3 w-3 text-gray-600 dark:text-gray-400" />
            )}
          </Button>
        </div>

        <Separator />

        {/* Specific notification types */}
        <div className="space-y-3">
          <h4 className="text-xs font-medium text-muted-foreground uppercase">
            Show notifications for:
          </h4>

          <div className="space-y-2">
            {[
              { key: 'showTaskCreations', label: 'New tasks', icon: Plus },
              { key: 'showTaskUpdates', label: 'Task updates', icon: Edit },
              { key: 'showAssignments', label: 'Task assignments', icon: UserCheck },
              { key: 'showCompletions', label: 'Task completions', icon: CheckCircle },
            ].map(({ key, label, icon: Icon }) => (
              <div key={key} className="flex items-center justify-between">
                <div className="flex items-center space-x-2">
                  <Icon className="h-3 w-3 text-muted-foreground" />
                  <span className="text-sm">{label}</span>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => toggleSetting(key as keyof NotificationSettings)}
                  disabled={!settings.enabled}
                  className="h-5 w-5 p-0"
                >
                  <div
                    className={`w-3 h-3 rounded-sm border ${
                      settings[key as keyof NotificationSettings] && settings.enabled
                        ? 'bg-primary border-primary'
                        : 'border-muted-foreground'
                    }`}
                  />
                </Button>
              </div>
            ))}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// Component that automatically handles notifications (should be placed in app root)
export function CollaborationNotificationHandler() {
  useCollaborationNotifications();
  return null;
}