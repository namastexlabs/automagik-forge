import { Link, useLocation } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import {
  FolderOpen,
  Settings,
  BookOpen,
  Server,
  MessageCircleQuestion,
  Github,
} from 'lucide-react';
import { Logo } from '@/components/logo';
import { UserMenu } from '@/components/ui/user-menu';
import { useAuth } from '@/components/auth-provider';

export function Navbar() {
  const location = useLocation();
  const { user, isAuthenticated } = useAuth();

  return (
    <div className="border-b">
      <div className="w-full px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          <div className="flex items-center space-x-6">
            <Link to="/">
              <Logo />
            </Link>
            <div className="flex items-center space-x-1">
              <Button
                asChild
                variant={
                  location.pathname === '/projects' ? 'default' : 'ghost'
                }
                size="sm"
              >
                <Link to="/projects">
                  <FolderOpen className="mr-2 h-4 w-4" />
                  Projects
                </Link>
              </Button>
              <Button
                asChild
                variant={
                  location.pathname === '/mcp-servers' ? 'default' : 'ghost'
                }
                size="sm"
              >
                <Link to="/mcp-servers">
                  <Server className="mr-2 h-4 w-4" />
                  MCP Servers
                </Link>
              </Button>
              <Button
                asChild
                variant={
                  location.pathname === '/settings' ? 'default' : 'ghost'
                }
                size="sm"
              >
                <Link to="/settings">
                  <Settings className="mr-2 h-4 w-4" />
                  Settings
                </Link>
              </Button>
            </div>
          </div>
          <div className="flex items-center space-x-1">
            <Button asChild variant="ghost" size="sm">
              <a
                href="/docs"
                target="_blank"
                rel="noopener noreferrer"
              >
                <BookOpen className="mr-2 h-4 w-4" />
                API Docs
              </a>
            </Button>
            <Button asChild variant="ghost" size="sm">
              <a
                href="https://github.com/namastexlabs/automagik-forge/issues"
                target="_blank"
                rel="noopener noreferrer"
              >
                <MessageCircleQuestion className="mr-2 h-4 w-4" />
                Support
              </a>
            </Button>
            {isAuthenticated && user ? (
              <UserMenu user={user} />
            ) : (
              <Button variant="ghost" size="sm">
                <Github className="mr-2 h-4 w-4" />
                Sign In
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
