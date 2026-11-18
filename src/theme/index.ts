import type { GlobalThemeOverrides } from 'naive-ui'

export type ThemeMode = 'light' | 'dark'
export const themeStorageKey = 'project-manage:theme'

export const themeColors = {
  primary: '#000000',
  primaryHover: '#262626',
  primaryPressed: '#000000',
  primarySoft: '#F5F5F5',
  primaryBorder: '#D4D4D4',
} as const

const darkThemeColors = {
  primary: '#FFFFFF',
  primaryHover: '#E5E5E5',
  primaryPressed: '#D4D4D4',
  primarySoft: '#262626',
  primaryBorder: '#525252',
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
    bodyColor: '#F8F8F8',
    cardColor: '#FFFFFF',
    modalColor: '#FFFFFF',
    popoverColor: '#FFFFFF',
    inputColor: '#FFFFFF',
    textColorBase: '#171717',
    textColor1: '#171717',
    textColor2: '#525252',
    textColor3: '#737373',
    placeholderColor: '#A3A3A3',
    borderColor: '#E5E5E5',
  },
}

export const darkThemeOverrides: GlobalThemeOverrides = {
  ...sharedOverrides,
  Button: {
    ...sharedOverrides.Button,
    textColorPrimary: '#000000',
    textColorHoverPrimary: '#000000',
    textColorPressedPrimary: '#000000',
    textColorFocusPrimary: '#000000',
    textColorDisabledPrimary: '#000000',
  },
  Checkbox: { checkMarkColor: '#000000' },
  common: {
    ...sharedOverrides.common,
    primaryColor: darkThemeColors.primary,
    primaryColorHover: darkThemeColors.primaryHover,
    primaryColorPressed: darkThemeColors.primaryPressed,
    primaryColorSuppl: darkThemeColors.primaryHover,
    bodyColor: '#101010',
    cardColor: '#1A1A1A',
    modalColor: '#1A1A1A',
    popoverColor: '#1A1A1A',
    inputColor: '#171717',
    textColorBase: '#E5E5E5',
    textColor1: '#E5E5E5',
    textColor2: '#D4D4D4',
    textColor3: '#A3A3A3',
    placeholderColor: '#737373',
    borderColor: '#333333',
    dividerColor: '#333333',
    hoverColor: '#262626',
    actionColor: '#171717',
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
  const colors = mode === 'dark' ? darkThemeColors : themeColors
  const variables = {
    '--accent': colors.primary,
    '--accent-hover': colors.primaryHover,
    '--accent-pressed': colors.primaryPressed,
    '--accent-soft': colors.primarySoft,
    '--accent-border': colors.primaryBorder,
    '--accent-contrast': mode === 'dark' ? '#000000' : '#FFFFFF',
  }
  for (const [key, value] of Object.entries(variables)) {
    root.style.setProperty(key, value)
  }
}
