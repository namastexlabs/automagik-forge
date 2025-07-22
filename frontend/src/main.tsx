import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App.tsx';
import './index.css';
import { ClickToComponent } from 'click-to-react-component';
import * as Sentry from '@sentry/react';
import {
  useLocation,
  useNavigationType,
  createRoutesFromChildren,
  matchRoutes,
} from 'react-router-dom';

// Only initialize Sentry if telemetry is not disabled
if (!import.meta.env.VITE_DISABLE_TELEMETRY) {
  Sentry.init({
    dsn: import.meta.env.VITE_SENTRY_DSN || "https://fa5e961d24021da4e6df30e5beee03af@o4509714066571264.ingest.us.sentry.io/4509714113495040",
    tracesSampleRate: 1.0,
    environment: import.meta.env.MODE === 'development' ? 'dev' : 'production',
    integrations: [
      Sentry.reactRouterV6BrowserTracingIntegration({
        useEffect: React.useEffect,
        useLocation,
        useNavigationType,
        createRoutesFromChildren,
        matchRoutes,
      }),
    ],
  });
  Sentry.setTag('source', 'frontend');
}

const AppContent = () => (
  <>
    <ClickToComponent />
    <App />
  </>
);

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {!import.meta.env.VITE_DISABLE_TELEMETRY ? (
      <Sentry.ErrorBoundary fallback={<p>An error has occurred</p>} showDialog>
        <AppContent />
      </Sentry.ErrorBoundary>
    ) : (
      <AppContent />
    )}
  </React.StrictMode>
);
