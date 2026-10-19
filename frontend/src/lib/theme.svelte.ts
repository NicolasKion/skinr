import { browser } from '$app/env';
import { THEME_COOKIE, THEME_MAX_AGE, type Theme } from '#lib/theme.js';

// The theme in effect. The server already put it on <html> as a class
// (hooks.server.ts), so the client starts from there.
class ThemeState {
  current = $state<Theme>(
    browser && document.documentElement.classList.contains('light') ? 'light' : 'dark',
  );

  set(theme: Theme) {
    this.current = theme;
    const root = document.documentElement;
    root.classList.remove('dark', 'light');
    root.classList.add(theme);
    document.cookie = `${THEME_COOKIE}=${theme}; path=/; max-age=${THEME_MAX_AGE}; samesite=lax`;
  }

  toggle() {
    this.set(this.current === 'dark' ? 'light' : 'dark');
  }
}

export const theme = new ThemeState();
