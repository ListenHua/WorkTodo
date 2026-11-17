// DOM physical key codes match the shortcut plugin's Code names across keyboard layouts.
export function shortcutFromKey(event: {
  code: string
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
  repeat: boolean
  isComposing: boolean
}): string | null {
  if (event.repeat || event.isComposing) return null
  const functionKey = /^F([1-9]|1\d|20)$/.test(event.code)
  const supported =
    /^(Key[A-Z]|Digit\d|Space|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right))$/.test(
      event.code,
    )
  const modifiers = [
    event.ctrlKey && 'Control',
    event.altKey && 'Alt',
    event.shiftKey && 'Shift',
    event.metaKey && 'Super',
  ].filter(Boolean)
  if ((!functionKey && !supported) || (!functionKey && !modifiers.length)) return null
  return [...modifiers, event.code].join('+')
}
