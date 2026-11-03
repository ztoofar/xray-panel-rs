import { createContext, useCallback, useContext, useLayoutEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { theme as antdTheme } from 'antd';
import type { ThemeConfig } from 'antd';

// v2: the panel now ships a dark sci-fi theme by default. The new key applies
// it once on upgrade without ignoring later theme toggles.
const STORAGE_DARK = 'dark-mode-v2';
const STORAGE_ULTRA = 'isUltraDarkThemeEnabled';

function readBool(key: string, fallback: boolean): boolean {
  const raw = localStorage.getItem(key);
  if (raw === null) return fallback;
  return raw === 'true';
}

function applyDom(isDark: boolean, isUltra: boolean) {
  document.body.classList.remove('dark', 'light');
  document.body.classList.add(isDark ? 'dark' : 'light');
  // Native scrollbars read color-scheme, not the body class.
  document.documentElement.style.colorScheme = isDark ? 'dark' : 'light';
  if (isUltra) {
    document.documentElement.setAttribute('data-theme', 'ultra-dark');
  } else {
    document.documentElement.removeAttribute('data-theme');
  }
  const msg = document.getElementById('message');
  if (msg) {
    msg.classList.remove('dark', 'light');
    msg.classList.add(isDark ? 'dark' : 'light');
  }
}

// module load so the document is in the right theme before React mounts.
const initialDark = readBool(STORAGE_DARK, true);
const initialUltra = readBool(STORAGE_ULTRA, false);
applyDom(initialDark, initialUltra);

const LIGHT_TOKENS = {
  colorBgBase: '#eef6fc',
  colorBgLayout: '#e8f3fb',
  colorBgContainer: '#ffffff',
  colorBgElevated: '#ffffff',
  colorText: '#0b2239',
  colorBorder: '#c9e2f2',
  colorBorderSecondary: 'rgba(8, 145, 178, 0.16)',
};
const DARK_TOKENS = {
  colorBgBase: '#050a12',
  colorBgLayout: '#050a12',
  colorBgContainer: '#0a1322',
  colorBgElevated: '#0e1a2e',
  colorText: '#d7e9ff',
  colorTextSecondary: '#9fbad6',
  colorBorder: '#1b3350',
  colorBorderSecondary: 'rgba(56, 189, 248, 0.14)',
};
const ULTRA_DARK_TOKENS = {
  colorBgBase: '#02040a',
  colorBgLayout: '#02040a',
  colorBgContainer: '#060c16',
  colorBgElevated: '#0a1322',
  colorText: '#cfe6ff',
  colorTextSecondary: '#93aec9',
  colorBorder: '#10233a',
  colorBorderSecondary: 'rgba(34, 211, 238, 0.10)',
};
const LIGHT_LAYOUT_TOKENS = {
  bodyBg: '#e8f3fb',
  headerBg: '#f4faff',
  headerColor: '#0b2239',
  footerBg: '#e8f3fb',
  siderBg: '#f4faff',
  triggerBg: '#e0f2fe',
  triggerColor: '#0b2239',
};
const DARK_LAYOUT_TOKENS = {
  bodyBg: '#050a12',
  headerBg: '#060d18',
  headerColor: '#d7e9ff',
  footerBg: '#050a12',
  siderBg: '#060d18',
  triggerBg: '#0e1a2e',
  triggerColor: '#d7e9ff',
};
const ULTRA_DARK_LAYOUT_TOKENS = {
  bodyBg: '#02040a',
  headerBg: '#02050b',
  headerColor: '#cfe6ff',
  footerBg: '#02040a',
  siderBg: '#02050b',
  triggerBg: '#0a1322',
  triggerColor: '#cfe6ff',
};
const LIGHT_MENU_TOKENS = {
  itemBg: '#f4faff',
  itemHoverBg: 'rgba(8, 145, 178, 0.07)',
  itemSelectedBg: 'rgba(8, 145, 178, 0.12)',
  itemSelectedColor: '#0e7490',
};
const DARK_MENU_TOKENS = {
  darkItemBg: '#060d18',
  darkSubMenuItemBg: '#050a12',
  darkPopupBg: '#0e1a2e',
  darkItemColor: 'rgba(215, 233, 255, 0.78)',
  darkItemHoverBg: 'rgba(34, 211, 238, 0.08)',
  darkItemSelectedBg: 'rgba(34, 211, 238, 0.12)',
  darkItemSelectedColor: '#67e8f9',
};
const ULTRA_DARK_MENU_TOKENS = {
  darkItemBg: '#02050b',
  darkSubMenuItemBg: '#02040a',
  darkPopupBg: '#060c16',
  darkItemColor: 'rgba(207, 230, 255, 0.72)',
  darkItemHoverBg: 'rgba(34, 211, 238, 0.06)',
  darkItemSelectedBg: 'rgba(34, 211, 238, 0.10)',
  darkItemSelectedColor: '#7dd3fc',
};
const LIGHT_CARD_TOKENS = {
  colorBorderSecondary: 'rgba(8, 145, 178, 0.16)',
};
const DARK_CARD_TOKENS = {
  colorBorderSecondary: 'rgba(56, 189, 248, 0.14)',
};
const ULTRA_DARK_CARD_TOKENS = {
  colorBorderSecondary: 'rgba(34, 211, 238, 0.10)',
};
const STATISTIC_TOKENS = {
  contentFontSize: 17,
  titleFontSize: 11,
};
const LIGHT_CONTRAST_TOKENS = {
  colorTextDescription: 'rgba(0, 0, 0, 0.58)',
  colorTextTertiary: 'rgba(0, 0, 0, 0.58)',
  colorTextPlaceholder: '#767676',
  colorError: '#cf1322',
  colorErrorText: '#cf1322',
  colorSuccessText: '#237804',
};
// Lumen accents: readable teal on the ice-light theme, electric cyan on dark.
const LIGHT_ACCENT_TOKENS = {
  colorPrimary: '#0891b2',
  colorPrimaryHover: '#06b6d4',
  colorPrimaryActive: '#0e7490',
  colorInfo: '#0284c7',
  colorSuccess: '#059669',
  colorWarning: '#d97706',
  colorError: '#dc2626',
  borderRadius: 10,
  borderRadiusLG: 14,
};
const DARK_ACCENT_TOKENS = {
  colorPrimary: '#22d3ee',
  colorPrimaryHover: '#67e8f9',
  colorPrimaryActive: '#06b6d4',
  colorInfo: '#38bdf8',
  colorSuccess: '#34d399',
  colorWarning: '#fbbf24',
  colorError: '#fb7185',
  borderRadius: 10,
  borderRadiusLG: 14,
};

// hashed:false drops the `:where(.css-<hash>)` wrapper antd puts around every
// rule. It costs nothing in specificity — `:where()` contributes zero, so the
// panel's own `.ant-*` overrides still win — and it removes roughly 5,700
// wrappers, 16% of the generated stylesheet, from what the browser has to parse.
//
// cssVar.key pins the CSS-variable scope. Every panel page mounts its own
// ConfigProvider (there is no root one), and without a fixed key each mints a
// fresh useId-derived scope, so navigating re-serialises and re-injects the whole
// token block under a new class instead of reusing the one already in the head.
const SHARED_STYLE_CONFIG = {
  hashed: false,
  cssVar: { key: 'xui' },
} as const;

export function buildAntdThemeConfig(isDark: boolean, isUltra: boolean): ThemeConfig {
  if (!isDark) {
    return {
      ...SHARED_STYLE_CONFIG,
      algorithm: antdTheme.defaultAlgorithm,
      token: { ...LIGHT_TOKENS, ...LIGHT_ACCENT_TOKENS, ...LIGHT_CONTRAST_TOKENS },
      components: {
        Layout: LIGHT_LAYOUT_TOKENS,
        Menu: LIGHT_MENU_TOKENS,
        Card: LIGHT_CARD_TOKENS,
        Statistic: STATISTIC_TOKENS,
      },
    };
  }
  return {
    ...SHARED_STYLE_CONFIG,
    algorithm: antdTheme.darkAlgorithm,
    token: {
      ...(isUltra ? ULTRA_DARK_TOKENS : DARK_TOKENS),
      ...DARK_ACCENT_TOKENS,
    },
    components: {
      Layout: isUltra ? ULTRA_DARK_LAYOUT_TOKENS : DARK_LAYOUT_TOKENS,
      Menu: isUltra ? ULTRA_DARK_MENU_TOKENS : DARK_MENU_TOKENS,
      Card: isUltra ? ULTRA_DARK_CARD_TOKENS : DARK_CARD_TOKENS,
      Statistic: STATISTIC_TOKENS,
    },
  };
}

export function pauseAnimationsUntilLeave(elementId: string): void {
  document.documentElement.setAttribute('data-theme-animations', 'off');
  const el = document.getElementById(elementId);
  if (!el) return;
  const restore = () => {
    document.documentElement.removeAttribute('data-theme-animations');
    el.removeEventListener('mouseleave', restore);
    el.removeEventListener('touchend', restore);
  };
  el.addEventListener('mouseleave', restore);
  el.addEventListener('touchend', restore);
}

interface ThemeContextValue {
  isDark: boolean;
  isUltra: boolean;
  toggleTheme: () => void;
  toggleUltra: () => void;
  antdThemeConfig: ThemeConfig;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [isDark, setIsDark] = useState<boolean>(initialDark);
  const [isUltra, setIsUltra] = useState<boolean>(initialUltra);

  useLayoutEffect(() => {
    applyDom(isDark, isUltra);
    localStorage.setItem(STORAGE_DARK, String(isDark));
    localStorage.setItem(STORAGE_ULTRA, String(isUltra));
  }, [isDark, isUltra]);

  const toggleTheme = useCallback(() => setIsDark((v) => !v), []);
  const toggleUltra = useCallback(() => setIsUltra((v) => !v), []);

  const antdThemeConfig = useMemo(() => buildAntdThemeConfig(isDark, isUltra), [isDark, isUltra]);

  const value = useMemo<ThemeContextValue>(
    () => ({ isDark, isUltra, toggleTheme, toggleUltra, antdThemeConfig }),
    [isDark, isUltra, toggleTheme, toggleUltra, antdThemeConfig],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used inside <ThemeProvider>');
  return ctx;
}
