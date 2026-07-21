export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";
export type AccentColor =
  | "indigo"
  | "cyan"
  | "emerald"
  | "rose"
  | "amber"
  | "violet";

export interface ThemePreferences {
  mode: ThemeMode;
  accent: AccentColor;
}

export const THEME_STORAGE_KEY = "video-tool.theme.preferences";

export const THEME_MODE_OPTIONS: ReadonlyArray<{
  value: ThemeMode;
  label: string;
  description: string;
}> = [
  {
    value: "system",
    label: "跟随系统",
    description: "根据操作系统浅色 / 深色自动切换",
  },
  {
    value: "light",
    label: "浅色",
    description: "始终使用浅色界面",
  },
  {
    value: "dark",
    label: "深色",
    description: "始终使用深色界面",
  },
];

export const ACCENT_COLOR_OPTIONS: ReadonlyArray<{
  value: AccentColor;
  label: string;
  swatch: string;
}> = [
  { value: "indigo", label: "靛蓝", swatch: "#6366f1" },
  { value: "cyan", label: "青色", swatch: "#06b6d4" },
  { value: "emerald", label: "翠绿", swatch: "#10b981" },
  { value: "rose", label: "玫红", swatch: "#f43f5e" },
  { value: "amber", label: "琥珀", swatch: "#f59e0b" },
  { value: "violet", label: "紫藤", swatch: "#8b5cf6" },
];

export const DEFAULT_THEME_PREFERENCES: ThemePreferences = {
  mode: "system",
  accent: "indigo",
};

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark";
}

function isAccentColor(value: unknown): value is AccentColor {
  return (
    value === "indigo" ||
    value === "cyan" ||
    value === "emerald" ||
    value === "rose" ||
    value === "amber" ||
    value === "violet"
  );
}

export function loadThemePreferences(): ThemePreferences {
  try {
    const rawValue = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (!rawValue) {
      return { ...DEFAULT_THEME_PREFERENCES };
    }

    const parsedValue = JSON.parse(rawValue) as Partial<ThemePreferences>;
    return {
      mode: isThemeMode(parsedValue.mode)
        ? parsedValue.mode
        : DEFAULT_THEME_PREFERENCES.mode,
      accent: isAccentColor(parsedValue.accent)
        ? parsedValue.accent
        : DEFAULT_THEME_PREFERENCES.accent,
    };
  } catch {
    return { ...DEFAULT_THEME_PREFERENCES };
  }
}

export function saveThemePreferences(preferences: ThemePreferences): void {
  window.localStorage.setItem(
    THEME_STORAGE_KEY,
    JSON.stringify({
      mode: preferences.mode,
      accent: preferences.accent,
    }),
  );
}

export function getSystemTheme(): ResolvedTheme {
  if (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: light)").matches
  ) {
    return "light";
  }
  return "dark";
}

export function resolveThemeMode(mode: ThemeMode): ResolvedTheme {
  if (mode === "system") {
    return getSystemTheme();
  }
  return mode;
}

export function applyThemePreferences(preferences: ThemePreferences): ResolvedTheme {
  const resolvedTheme = resolveThemeMode(preferences.mode);
  const rootElement = document.documentElement;

  rootElement.dataset.theme = resolvedTheme;
  rootElement.dataset.accent = preferences.accent;
  rootElement.style.colorScheme = resolvedTheme;

  return resolvedTheme;
}
