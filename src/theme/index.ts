import type { GlobalThemeOverrides } from 'naive-ui'

export type ThemeMode = 'light' | 'dark'
export const themeStorageKey = 'project-manage:theme'

export const themeColors = {
  primary: '#1677FF',
  primaryHover: '#4096FF',
  primaryPressed: '#0958D9',
  primarySoft: '#E6F4FF',
  primaryBorder: '#BAE0FF',
} as const

const sharedOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: themeColors.primary,
    primaryColorHover: themeColors.primaryHover,
    primaryColorPressed: themeColors.primaryPressed,
    primaryColorSuppl: themeColors.primaryHover,
    borderRadius: '7px',
    fontSize: '14px',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif',
  },
  Button: {
    heightMedium: '34px',
    heightLarge: '40px',
    borderRadiusMedium: '7px',
    borderRadiusLarge: '7px',
    fontWeight: '400',
    textColorPrimary: '#FFFFFF',
    textColorHoverPrimary: '#FFFFFF',
    textColorPressedPrimary: '#FFFFFF',
    textColorFocusPrimary: '#FFFFFF',
    textColorDisabledPrimary: '#FFFFFF',
  },
  Card: { borderRadius: '12px' },
  Checkbox: { checkMarkColor: '#FFFFFF' },
}

export const themeOverrides: GlobalThemeOverrides = {
  ...sharedOverrides,
  common: {
    ...sharedOverrides.common,
    bodyColor: '#F5F7FB',
    cardColor: '#FFFFFF',
    modalColor: '#FFFFFF',
    popoverColor: '#FFFFFF',
    inputColor: '#FFFFFF',
    textColorBase: '#1F2937',
    textColor1: '#1F2937',
    textColor2: '#475569',
    textColor3: '#64748B',
    placeholderColor: '#94A3B8',
    borderColor: '#E2E8F0',
  },
}

export const darkThemeOverrides: GlobalThemeOverrides = {
  ...sharedOverrides,
  common: {
    ...sharedOverrides.common,
    bodyColor: '#0F1624',
    cardColor: '#182235',
    modalColor: '#182235',
    popoverColor: '#182235',
    inputColor: '#141D2E',
    textColorBase: '#E2E8F0',
    textColor1: '#E2E8F0',
    textColor2: '#CBD5E1',
    textColor3: '#94A3B8',
    placeholderColor: '#76869E',
    borderColor: '#334155',
    dividerColor: '#334155',
    hoverColor: '#223149',
    actionColor: '#141D2E',
  },
}

export function readThemePreference(): ThemeMode | 'system' {
  try {
    const saved = window.localStorage.getItem(themeStorageKey)
    return saved === 'dark' || saved === 'light' ? saved : 'system'
  } catch {
    return 'light'
  }
}

// Root variables also reach menus and modals teleported outside the app container.
export function applyThemeVariables(mode: ThemeMode): void {
  const root = document.documentElement
  root.classList.toggle('dark', mode === 'dark')
  root.dataset.theme = mode
  root.style.colorScheme = mode
  const variables = {
    '--accent': themeColors.primary,
    '--accent-hover': themeColors.primaryHover,
    '--accent-pressed': themeColors.primaryPressed,
    '--accent-soft': mode === 'dark' ? '#112D4D' : themeColors.primarySoft,
    '--accent-border': mode === 'dark' ? '#274B73' : themeColors.primaryBorder,
  }
  for (const [key, value] of Object.entries(variables)) {
    root.style.setProperty(key, value)
  }
}
