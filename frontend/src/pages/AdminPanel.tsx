import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Loader } from '@/components/ui/loader';
import { Shield, UserPlus, UserX, Users } from 'lucide-react';
import { useAuth } from '@/components/auth-provider';
import type { GitHubWhitelist, User } from 'shared/types';

// This would need proper API functions - placeholder for now
const whitelistApi = {
  getAll: async (): Promise<GitHubWhitelist[]> => {
    // TODO: Implement API call
    return [];
  },
  add: async (_username: string): Promise<GitHubWhitelist> => {
    // TODO: Implement API call
    throw new Error('Not implemented');
  },
  remove: async (_id: string): Promise<void> => {
    // TODO: Implement API call
    throw new Error('Not implemented');
  },
};

export function AdminPanel() {
  const { user } = useAuth();
  const [whitelist, setWhitelist] = useState<GitHubWhitelist[]>([]);
  const [users, _setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(true);
  const [newUsername, setNewUsername] = useState('');
  const [adding, setAdding] = useState(false);

  // Redirect if not admin
  if (!user?.is_admin) {
    return (
      <div className="container mx-auto px-4 py-8">
        <div className="text-center">
          <Shield className="h-16 w-16 mx-auto text-muted-foreground mb-4" />
          <h1 className="text-2xl font-bold mb-2">Access Denied</h1>
          <p className="text-muted-foreground">
            You need administrator privileges to access this page.
          </p>
        </div>
      </div>
    );
  }

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      try {
        // TODO: Implement proper API calls
        // const [whitelistData, usersData] = await Promise.all([
        //   whitelistApi.getAll(),
        //   authApi.getUsers(),
        // ]);
        // setWhitelist(whitelistData);
        // setUsers(usersData);
      } catch (error) {
        console.error('Failed to fetch admin data:', error);
      } finally {
        setLoading(false);
      }
    };

    fetchData();
  }, []);

  const handleAddUser = async () => {
    if (!newUsername.trim()) return;

    setAdding(true);
    try {
      const newEntry = await whitelistApi.add(newUsername.trim());
      setWhitelist([...whitelist, newEntry]);
      setNewUsername('');
    } catch (error) {
      console.error('Failed to add user to whitelist:', error);
    } finally {
      setAdding(false);
    }
  };

  const handleRemoveUser = async (id: string) => {
    try {
      await whitelistApi.remove(id);
      setWhitelist(whitelist.filter((entry) => entry.id !== id));
    } catch (error) {
      console.error('Failed to remove user from whitelist:', error);
    }
  };

  if (loading) {
    return (
      <div className="container mx-auto px-4 py-8">
        <Loader message="Loading admin panel..." size={32} />
      </div>
    );
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="mb-8">
        <h1 className="text-3xl font-bold flex items-center gap-3">
          <Shield className="h-8 w-8" />
          Admin Panel
        </h1>
        <p className="text-muted-foreground mt-2">
          Manage team access and user permissions
        </p>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        {/* User Management */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Users className="h-5 w-5" />
              Team Members ({users.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {users.length === 0 ? (
                <p className="text-muted-foreground text-sm">
                  No team members found.
                </p>
              ) : (
                users.map((teamUser) => (
                  <div
                    key={teamUser.id}
                    className="flex items-center justify-between p-3 border rounded-lg"
                  >
                    <div className="flex items-center space-x-3">
                      <div>
                        <div className="font-medium">
                          {teamUser.display_name || teamUser.username}
                        </div>
                        <div className="text-sm text-muted-foreground">
                          {teamUser.email}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center space-x-2">
                      {teamUser.is_admin && (
                        <span className="text-xs bg-primary text-primary-foreground px-2 py-1 rounded">
                          Admin
                        </span>
                      )}
                      {teamUser.is_whitelisted && (
                        <span className="text-xs bg-green-100 text-green-800 px-2 py-1 rounded">
                          Active
                        </span>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          </CardContent>
        </Card>

        {/* Whitelist Management */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <UserPlus className="h-5 w-5" />
              GitHub Whitelist
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="flex space-x-2">
                <Input
                  placeholder="GitHub username"
                  value={newUsername}
                  onChange={(e) => setNewUsername(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      handleAddUser();
                    }
                  }}
                />
                <Button
                  onClick={handleAddUser}
                  disabled={adding || !newUsername.trim()}
                >
                  {adding ? 'Adding...' : 'Add'}
                </Button>
              </div>

              <div className="space-y-2">
                {whitelist.length === 0 ? (
                  <p className="text-muted-foreground text-sm">
                    No whitelisted users found.
                  </p>
                ) : (
                  whitelist.map((entry) => (
                    <div
                      key={entry.id}
                      className="flex items-center justify-between p-3 border rounded-lg"
                    >
                      <div>
                        <div className="font-medium">{entry.github_username}</div>
                        {entry.notes && (
                          <div className="text-sm text-muted-foreground">
                            {entry.notes}
                          </div>
                        )}
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleRemoveUser(entry.id)}
                      >
                        <UserX className="h-4 w-4" />
                      </Button>
                    </div>
                  ))
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}