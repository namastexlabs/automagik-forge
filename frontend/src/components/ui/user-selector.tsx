import React, { useEffect, useState } from 'react';
import { Check, ChevronsUpDown, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
} from '@/components/ui/command';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { UserAvatar } from '@/components/ui/user-avatar';
import { authApi } from '@/lib/api';
import type { User } from 'shared/types';

interface UserSelectorProps {
  value?: string | null;
  onValueChange: (value: string | null) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}

export function UserSelector({
  value,
  onValueChange,
  placeholder = 'Select assignee...',
  disabled = false,
  className = '',
}: UserSelectorProps) {
  const [open, setOpen] = useState(false);
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(false);

  const selectedUser = users.find((user) => user.id === value);

  useEffect(() => {
    const fetchUsers = async () => {
      setLoading(true);
      try {
        const fetchedUsers = await authApi.getUsers();
        setUsers(fetchedUsers);
      } catch (error) {
        console.error('Failed to fetch users:', error);
      } finally {
        setLoading(false);
      }
    };

    if (open && users.length === 0) {
      fetchUsers();
    }
  }, [open, users.length]);

  const handleSelect = (userId: string) => {
    if (userId === value) {
      onValueChange(null); // Deselect if clicking the same user
    } else {
      onValueChange(userId);
    }
    setOpen(false);
  };

  const handleClear = (e: React.MouseEvent) => {
    e.stopPropagation();
    onValueChange(null);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          disabled={disabled}
          className={`justify-between ${className}`}
        >
          {selectedUser ? (
            <div className="flex items-center space-x-2">
              <UserAvatar
                src={selectedUser.avatar_url}
                alt={selectedUser.display_name || selectedUser.username}
                size="sm"
              />
              <span className="truncate">
                {selectedUser.display_name || selectedUser.username}
              </span>
            </div>
          ) : (
            <span className="text-muted-foreground">{placeholder}</span>
          )}
          <div className="flex items-center space-x-1">
            {selectedUser && (
              <X
                className="h-4 w-4 opacity-50 hover:opacity-100"
                onClick={handleClear}
              />
            )}
            <ChevronsUpDown className="h-4 w-4 shrink-0 opacity-50" />
          </div>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[300px] p-0">
        <Command>
          <CommandInput placeholder="Search users..." />
          <CommandEmpty>
            {loading ? 'Loading users...' : 'No users found.'}
          </CommandEmpty>
          <CommandGroup>
            {users.map((user) => (
              <CommandItem
                key={user.id}
                value={`${user.username} ${user.display_name || ''} ${user.email}`}
                onSelect={() => handleSelect(user.id)}
                className="flex items-center space-x-2"
              >
                <UserAvatar
                  src={user.avatar_url}
                  alt={user.display_name || user.username}
                  size="sm"
                />
                <div className="flex-1 min-w-0">
                  <div className="font-medium truncate">
                    {user.display_name || user.username}
                  </div>
                  <div className="text-sm text-muted-foreground truncate">
                    {user.email}
                  </div>
                </div>
                <Check
                  className={`h-4 w-4 ${
                    value === user.id ? 'opacity-100' : 'opacity-0'
                  }`}
                />
              </CommandItem>
            ))}
          </CommandGroup>
        </Command>
      </PopoverContent>
    </Popover>
  );
}