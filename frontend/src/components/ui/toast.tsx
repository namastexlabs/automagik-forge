import React, { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { X } from 'lucide-react';
import { Button } from './button';

interface Toast {
  id: string;
  content: React.ReactNode;
  duration?: number;
  type?: 'info' | 'success' | 'warning' | 'error';
}

interface ToastContextType {
  toasts: Toast[];
  addToast: (content: React.ReactNode, options?: { duration?: number; type?: Toast['type'] }) => string;
  removeToast: (id: string) => void;
}

const ToastContext = createContext<ToastContextType | undefined>(undefined);

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider');
  }
  return context;
}

interface ToastProviderProps {
  children: React.ReactNode;
}

export function ToastProvider({ children }: ToastProviderProps) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback((
    content: React.ReactNode, 
    options: { duration?: number; type?: Toast['type'] } = {}
  ) => {
    const id = Math.random().toString(36).substr(2, 9);
    const toast: Toast = {
      id,
      content,
      duration: options.duration || 4000,
      type: options.type || 'info',
    };

    setToasts(prev => [...prev, toast]);

    // Auto remove after duration
    if (toast.duration > 0) {
      setTimeout(() => {
        removeToast(id);
      }, toast.duration);
    }

    return id;
  }, []);

  const removeToast = useCallback((id: string) => {
    setToasts(prev => prev.filter(toast => toast.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ toasts, addToast, removeToast }}>
      {children}
      <ToastContainer />
    </ToastContext.Provider>
  );
}

function ToastContainer() {
  const { toasts, removeToast } = useToast();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-50 flex flex-col space-y-2 max-w-sm">
      {toasts.map(toast => (
        <ToastItem
          key={toast.id}
          toast={toast}
          onRemove={() => removeToast(toast.id)}
        />
      ))}
    </div>
  );
}

interface ToastItemProps {
  toast: Toast;
  onRemove: () => void;
}

function ToastItem({ toast, onRemove }: ToastItemProps) {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    // Animate in
    const timer = setTimeout(() => setIsVisible(true), 50);
    return () => clearTimeout(timer);
  }, []);

  const handleRemove = () => {
    setIsVisible(false);
    setTimeout(onRemove, 200); // Allow animation to complete
  };

  const getTypeStyles = () => {
    switch (toast.type) {
      case 'success':
        return 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800';
      case 'warning':
        return 'bg-yellow-50 dark:bg-yellow-900/20 border-yellow-200 dark:border-yellow-800';
      case 'error':
        return 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800';
      default:
        return 'bg-background border-border';
    }
  };

  return (
    <div
      className={`
        relative rounded-lg border p-4 shadow-lg transition-all duration-200 ease-in-out
        ${getTypeStyles()}
        ${isVisible 
          ? 'transform translate-x-0 opacity-100' 
          : 'transform translate-x-full opacity-0'
        }
      `}
    >
      <div className="flex items-start space-x-3">
        <div className="flex-1">
          {toast.content}
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleRemove}
          className="h-6 w-6 p-0 hover:bg-muted"
        >
          <X className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
}

// Convenience function for creating toasts
export const toast = {
  info: (content: React.ReactNode, duration?: number) => {
    // This will be replaced by the actual implementation when used within ToastProvider
    console.log('Toast (info):', content);
  },
  success: (content: React.ReactNode, duration?: number) => {
    console.log('Toast (success):', content);
  },
  warning: (content: React.ReactNode, duration?: number) => {
    console.log('Toast (warning):', content);
  },
  error: (content: React.ReactNode, duration?: number) => {
    console.log('Toast (error):', content);
  },
};