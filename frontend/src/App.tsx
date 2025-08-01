import { useEffect, useState } from 'react';
import { BrowserRouter, Route, Routes } from 'react-router-dom';
import { Navbar } from '@/components/layout/navbar';
import { Projects } from '@/pages/projects';
import { ProjectTasks } from '@/pages/project-tasks';
import { Settings } from '@/pages/Settings';
import { McpServers } from '@/pages/McpServers';
import { AdminPanel } from '@/pages/AdminPanel';
import { DisclaimerDialog } from '@/components/DisclaimerDialog';
import { OnboardingDialog } from '@/components/OnboardingDialog';
import { ConfigProvider, useConfig } from '@/components/config-provider';
import { AuthProvider, useAuth } from '@/components/auth-provider';
import { ThemeProvider } from '@/components/theme-provider';
import { ToastProvider } from '@/components/ui/toast';
import type { EditorType, ExecutorConfig } from 'shared/types';
import { configApi } from '@/lib/api';
import * as Sentry from '@sentry/react';
import { Loader } from '@/components/ui/loader';
import { GitHubLoginDialog } from '@/components/GitHubLoginDialog';

const SentryRoutes = Sentry.withSentryReactRouterV6Routing(Routes);

function AppContent() {
  const { config, updateConfig, loading } = useConfig();
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const [showDisclaimer, setShowDisclaimer] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [showGitHubLogin, setShowGitHubLogin] = useState(false);
  const showNavbar = true;

  useEffect(() => {
    if (config && !authLoading) {
      console.log('App.tsx useEffect:', { config, authLoading, isAuthenticated, showGitHubLogin });
      
      // Always show GitHub login if user is not authenticated, regardless of config flags
      if (!isAuthenticated) {
        console.log('User not authenticated, showing GitHub login');
        setShowGitHubLogin(true);
        return;
      }
      
      // If authenticated, go through normal onboarding flow
      console.log('User authenticated, going through onboarding flow');
      setShowGitHubLogin(false); // Make sure GitHub login is hidden when authenticated
      setShowDisclaimer(!config.disclaimer_acknowledged);
      if (config.disclaimer_acknowledged) {
        setShowOnboarding(!config.onboarding_acknowledged);
      }
    }
  }, [config, authLoading, isAuthenticated]);

  const handleDisclaimerAccept = async () => {
    if (!config) return;

    updateConfig({ disclaimer_acknowledged: true });

    try {
      await configApi.saveConfig({ ...config, disclaimer_acknowledged: true });
      setShowDisclaimer(false);
      setShowOnboarding(!config.onboarding_acknowledged);
    } catch (err) {
      console.error('Error saving config:', err);
    }
  };

  const handleOnboardingComplete = async (onboardingConfig: {
    executor: ExecutorConfig;
    editor: { editor_type: EditorType; custom_command: string | null };
  }) => {
    if (!config) return;

    const updatedConfig = {
      ...config,
      onboarding_acknowledged: true,
      executor: onboardingConfig.executor,
      editor: onboardingConfig.editor,
    };

    updateConfig(updatedConfig);

    try {
      await configApi.saveConfig(updatedConfig);
      setShowOnboarding(false);
    } catch (err) {
      console.error('Error saving config:', err);
    }
  };


  const handleGitHubLoginComplete = async () => {
    try {
      // Refresh the config to get the latest GitHub authentication state
      const latestConfig = await configApi.getConfig();
      updateConfig(latestConfig);
      setShowGitHubLogin(false);

      // If user completed GitHub login, the AuthProvider should have updated
      // If user skipped, we need to manually set the acknowledgment
      const updatedConfig = {
        ...latestConfig,
        github_login_acknowledged: true,
      };
      updateConfig(updatedConfig);
      await configApi.saveConfig(updatedConfig);
    } catch (err) {
      console.error('Error refreshing config:', err);
    }
  };

  if (loading || authLoading) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <Loader message="Loading..." size={32} />
      </div>
    );
  }

  // If not authenticated and not showing GitHub login dialog, show loading
  if (!isAuthenticated && !showGitHubLogin) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <Loader message="Please sign in..." size={32} />
      </div>
    );
  }

  return (
    <ThemeProvider initialTheme={config?.theme || 'system'}>
      <div className="h-screen flex flex-col bg-background">
        <GitHubLoginDialog
          open={showGitHubLogin}
          onOpenChange={handleGitHubLoginComplete}
        />
        <DisclaimerDialog
          open={showDisclaimer}
          onAccept={handleDisclaimerAccept}
        />
        <OnboardingDialog
          open={showOnboarding}
          onComplete={handleOnboardingComplete}
        />
        {showNavbar && isAuthenticated && <Navbar />}
        <div className="flex-1 overflow-y-scroll">
          {isAuthenticated && (
            <SentryRoutes>
              <Route path="/" element={<Projects />} />
              <Route path="/projects" element={<Projects />} />
              <Route path="/projects/:projectId" element={<Projects />} />
              <Route
                path="/projects/:projectId/tasks"
                element={<ProjectTasks />}
              />
              <Route
                path="/projects/:projectId/tasks/:taskId"
                element={<ProjectTasks />}
              />

              <Route path="/settings" element={<Settings />} />
              <Route path="/mcp-servers" element={<McpServers />} />
              <Route path="/admin" element={<AdminPanel />} />
            </SentryRoutes>
          )}
        </div>
      </div>
    </ThemeProvider>
  );
}

function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <ConfigProvider>
          <ToastProvider>
            <AppContent />
          </ToastProvider>
        </ConfigProvider>
      </AuthProvider>
    </BrowserRouter>
  );
}

export default App;
