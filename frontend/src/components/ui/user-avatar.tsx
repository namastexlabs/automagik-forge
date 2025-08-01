import { User } from 'lucide-react';

interface UserAvatarProps {
  src?: string | null;
  alt?: string;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export function UserAvatar({ 
  src, 
  alt, 
  size = 'md', 
  className = ''
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

  return src ? (
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
}