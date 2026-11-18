export function globalShortcutDefaults(isMacos: boolean) {
  const modifiers = isMacos ? 'Control+Super' : 'Control+Alt'
  return { shortcut: `${modifiers}+KeyD`, toggleShortcut: `${modifiers}+KeyE` }
}

export function normalizeShortcut(value: string, isMacos: boolean) {
  const parts = value
    .trim()
    .split('+')
    .map((part) => part.trim().toUpperCase())
  const key = parts.pop() || ''
  const modifiers = new Set(
    parts.map((part) => {
      if (['CTRL', 'CONTROL'].includes(part)) return 'CTRL'
      if (['OPTION', 'ALT'].includes(part)) return 'ALT'
      if (['CMD', 'COMMAND', 'SUPER', 'META', 'WIN', 'WINDOWS'].includes(part)) return 'SUPER'
      if (['COMMANDORCONTROL', 'COMMANDORCTRL', 'CMDORCTRL', 'CMDORCONTROL'].includes(part))
        return isMacos ? 'SUPER' : 'CTRL'
      return part
    }),
  )
  return [
    ...['CTRL', 'ALT', 'SHIFT', 'SUPER'].filter((part) => modifiers.has(part)),
    key.replace(/^(KEY|DIGIT)/, ''),
  ].join('+')
}

export function shortcutLabel(value: string, isMacos: boolean) {
  if (!value.trim()) return '未设置'
  return normalizeShortcut(value, isMacos)
    .split('+')
    .map((part) => {
      if (part === 'CTRL') return 'Control'
      if (part === 'ALT') return isMacos ? 'Option' : 'Alt'
      if (part === 'SUPER') return isMacos ? 'Command' : 'Win'
      if (part === 'SHIFT') return 'Shift'
      if (part === 'SPACE') return 'Space'
      return part
    })
    .join(' + ')
}

export function globalShortcutConflict(
  settings: {
    shortcut: string
    toggleShortcut: string
    captureTrigger: string
  },
  isMacos: boolean,
) {
  const bindings = [
    { name: '快捷添加待办', value: settings.shortcut },
    { name: '显示/隐藏窗口', value: settings.toggleShortcut },
    {
      name: '快捷采集',
      value: settings.captureTrigger.startsWith('Mouse') ? '' : settings.captureTrigger,
    },
  ].filter((item) => item.value.trim())
  const copy = normalizeShortcut('CommandOrControl+C', isMacos)
  for (let index = 0; index < bindings.length; index++) {
    const binding = bindings[index]!
    const key = normalizeShortcut(binding.value, isMacos)
    if (key === copy) return `${binding.name}不能占用系统复制快捷键`
    const other = bindings
      .slice(0, index)
      .find((item) => normalizeShortcut(item.value, isMacos) === key)
    if (other) return `${binding.name}与${other.name}快捷键相同，请更换组合`
  }
  return ''
}
