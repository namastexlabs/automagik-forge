import { cn } from '@/lib/utils';
import type { SSEConnectionState } from '@/hooks/useSSEStream';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

export interface ConnectionHealthIndicatorProps {
  /**
   * Current connection state
   */
  state: SSEConnectionState;
  /**
   * Number of reconnection attempts (shown in tooltip)
   */
  reconnectAttempts?: number;
  /**
   * Error message (shown in tooltip when disconnected)
   */
  error?: string | null;
  /**
   * Last event ID (shown in tooltip for debugging)
   */
  lastEventId?: string | null;
  /**
   * Size of the indicator
   */
  size?: 'sm' | 'md' | 'lg';
  /**
   * Whether to show the label text
   */
  showLabel?: boolean;
  /**
   * Additional CSS classes
   */
  className?: string;
  /**
   * Callback when clicked (e.g., to manually reconnect)
   */
  onClick?: () => void;
}

const sizeClasses = {
  sm: 'h-2 w-2',
  md: 'h-2.5 w-2.5',
  lg: 'h-3 w-3',
};

const labelSizeClasses = {
  sm: 'text-[10px]',
  md: 'text-xs',
  lg: 'text-sm',
};

interface StateConfig {
  color: string;
  pulseColor: string;
  label: string;
  animate: boolean;
}

const stateConfigs: Record<SSEConnectionState, StateConfig> = {
  connected: {
    color: 'bg-green-500',
    pulseColor: 'bg-green-400',
    label: 'Connected',
    animate: false,
  },
  connecting: {
    color: 'bg-yellow-500',
    pulseColor: 'bg-yellow-400',
    label: 'Connecting',
    animate: true,
  },
  reconnecting: {
    color: 'bg-yellow-500',
    pulseColor: 'bg-yellow-400',
    label: 'Reconnecting',
    animate: true,
  },
  disconnected: {
    color: 'bg-red-500',
    pulseColor: 'bg-red-400',
    label: 'Disconnected',
    animate: false,
  },
};

/**
 * Visual indicator component for SSE connection health.
 * Shows connected/reconnecting/disconnected state with appropriate
 * colors and animations. Includes tooltip with detailed status info.
 *
 * @example
 * ```tsx
 * <ConnectionHealthIndicator
 *   state={connectionState}
 *   reconnectAttempts={reconnectAttempts}
 *   error={error}
 *   showLabel={true}
 *   onClick={reconnect}
 * />
 * ```
 */
export function ConnectionHealthIndicator({
  state,
  reconnectAttempts = 0,
  error,
  lastEventId,
  size = 'md',
  showLabel = false,
  className,
  onClick,
}: ConnectionHealthIndicatorProps) {
  const config = stateConfigs[state];
  const isClickable = onClick && state === 'disconnected';

  // Build tooltip content
  const tooltipLines: string[] = [config.label];

  if (state === 'reconnecting' && reconnectAttempts > 0) {
    tooltipLines.push(`Attempt ${reconnectAttempts}`);
  }

  if (error && state === 'disconnected') {
    tooltipLines.push(error);
  }

  if (lastEventId) {
    tooltipLines.push(`Last ID: ${lastEventId}`);
  }

  if (isClickable) {
    tooltipLines.push('Click to reconnect');
  }

  const indicator = (
    <div
      className={cn(
        'inline-flex items-center gap-1.5',
        isClickable && 'cursor-pointer hover:opacity-80',
        className
      )}
      onClick={isClickable ? onClick : undefined}
      role={isClickable ? 'button' : undefined}
      tabIndex={isClickable ? 0 : undefined}
      onKeyDown={
        isClickable
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onClick?.();
              }
            }
          : undefined
      }
      aria-label={tooltipLines.join('. ')}
    >
      {/* Indicator dot with optional pulse animation */}
      <span className="relative flex">
        {config.animate && (
          <span
            className={cn(
              'absolute inline-flex h-full w-full rounded-full opacity-75 animate-ping',
              config.pulseColor
            )}
          />
        )}
        <span
          className={cn(
            'relative inline-flex rounded-full',
            sizeClasses[size],
            config.color
          )}
        />
      </span>

      {/* Optional label */}
      {showLabel && (
        <span
          className={cn(
            'font-medium',
            labelSizeClasses[size],
            state === 'connected' && 'text-green-600 dark:text-green-400',
            state === 'connecting' && 'text-yellow-600 dark:text-yellow-400',
            state === 'reconnecting' && 'text-yellow-600 dark:text-yellow-400',
            state === 'disconnected' && 'text-red-600 dark:text-red-400'
          )}
        >
          {config.label}
          {state === 'reconnecting' && reconnectAttempts > 0 && (
            <span className="opacity-70 ml-1">({reconnectAttempts})</span>
          )}
        </span>
      )}
    </div>
  );

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>{indicator}</TooltipTrigger>
        <TooltipContent side="bottom" className="max-w-xs">
          <div className="flex flex-col gap-0.5">
            {tooltipLines.map((line, i) => (
              <span key={i} className={i === 0 ? 'font-medium' : 'text-muted-foreground'}>
                {line}
              </span>
            ))}
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

export default ConnectionHealthIndicator;
