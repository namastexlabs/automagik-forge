import { User } from 'lucide-react';
import { PresenceStatus } from 'shared/types';

interface UserAvatarProps {
  src?: string | null;
  alt?: string;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
  presenceStatus?: PresenceStatus;
  showPresence?: boolean;
}

export function UserAvatar({ 
  src, 
  alt, 
  size = 'md', 
  className = '', 
  presenceStatus,
  showPresence = false 
}: UserAvatarProps) {
  const sizeClasses = {
    sm: 'w-6 h-6',
    md: 'w-8 h-8',
    lg: 'w-12 h-12'
  };

  const iconSizes = {
    sm: 'w-3 h-3',
    md: 'w-4 h-4',
    lg: 'w-6 h-6'
  };

  const presenceSizes = {
    sm: 'w-2 h-2',
    md: 'w-2.5 h-2.5',
    lg: 'w-3 h-3'
  };

  const getPresenceColor = (status?: PresenceStatus) => {
    switch (status) {
      case 'Online':
        return 'bg-green-500 animate-pulse';
      case 'Away':
        return 'bg-yellow-500';
      case 'Offline':
        return 'bg-gray-400';
      default:
        return 'bg-gray-400';
    }
  };

  const avatarContent = src ? (
    <>
      <img
        src={src}
        alt={alt || 'User avatar'}
        className={`${sizeClasses[size]} rounded-full border border-border bg-muted ${className}`}
        onError={(e) => {
          // If image fails to load, hide it and show fallback
          e.currentTarget.style.display = 'none';
          const fallback = e.currentTarget.nextElementSibling as HTMLElement;
          if (fallback) {
            fallback.style.display = 'flex';
          }
        }}
      />
      <div
        className={`${sizeClasses[size]} rounded-full border border-border bg-muted flex items-center justify-center ${className}`}
        style={{ display: 'none' }}
      >
        <User className={`${iconSizes[size]} text-muted-foreground`} />
      </div>
    </>
  ) : (
    <div
      className={`${sizeClasses[size]} rounded-full border border-border bg-muted flex items-center justify-center ${className}`}
    >
      <User className={`${iconSizes[size]} text-muted-foreground`} />
    </div>
  );

  if (!showPresence || !presenceStatus) {
    return avatarContent;
  }

  return (
    <div className="relative inline-block">
      {avatarContent}
      <div 
        className={`absolute -bottom-0.5 -right-0.5 ${presenceSizes[size]} ${getPresenceColor(presenceStatus)} rounded-full border-2 border-background`}
      />
    </div>
  );
}