import { useTheme } from '@/components/theme-provider';
import { useEffect, useState } from 'react';

export function Logo({ className = '' }: { className?: string }) {
  const { theme } = useTheme();
  const [isDark, setIsDark] = useState(false);

  useEffect(() => {
    const updateTheme = () => {
      if (theme === 'light') {
        setIsDark(false);
      } else if (theme === 'system') {
        // System theme
        setIsDark(window.matchMedia('(prefers-color-scheme: dark)').matches);
      } else {
        // All other themes (dark, purple, green, blue, orange, red) have dark backgrounds
        setIsDark(true);
      }
    };

    updateTheme();

    // Listen for system theme changes when using system theme
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      mediaQuery.addEventListener('change', updateTheme);
      return () => mediaQuery.removeEventListener('change', updateTheme);
    }
  }, [theme]);

  // Use appropriate logo based on theme
  const logoSrc = isDark ? '/automagik-forge-logo-dark.svg' : '/automagik-forge-logo.svg';

  return (
    <img
      src={logoSrc}
      alt="AUTOMAGIK FORGE"
      width="350"
      className={`${className} h-auto`}
      style={{ maxWidth: '450px', height: 'auto' }}
    />
  );
}
