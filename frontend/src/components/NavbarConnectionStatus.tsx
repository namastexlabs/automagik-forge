import { useGlobalConnectionStatus } from '@/hooks/useGlobalStreamStatus';
import { ConnectionHealthIndicator } from '@/components/ui/ConnectionHealthIndicator';

/**
 * Connection status indicator for the navbar.
 * Shows global real-time stream connection health.
 * Only renders when there are active streams being tracked.
 */
export function NavbarConnectionStatus() {
  const { state, connectedCount, totalCount, error, isEmpty } = useGlobalConnectionStatus();

  // Don't render if no streams are registered
  if (isEmpty) {
    return null;
  }

  return (
    <div className="hidden sm:flex items-center ml-2">
      <ConnectionHealthIndicator
        state={state}
        error={error}
        size="sm"
        showLabel={false}
      />
      {totalCount > 1 && (
        <span className="ml-1 text-[10px] text-muted-foreground">
          {connectedCount}/{totalCount}
        </span>
      )}
    </div>
  );
}

export default NavbarConnectionStatus;
