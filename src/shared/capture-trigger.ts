export const mouseTriggers = [
  { label: '鼠标中键', value: 'MouseMiddle' },
  { label: '鼠标右键', value: 'MouseRight' },
  { label: '鼠标侧键（后退）', value: 'MouseBack' },
  { label: '鼠标侧键（前进）', value: 'MouseForward' },
  { label: '鼠标左键', value: 'MouseLeft' },
]
export function isMouseTrigger(trigger: string) {
  return (
    mouseTriggers.some((item) => item.value === trigger) ||
    /^MouseButton(?:[6-9]|[12]\d|3[0-2])$/.test(trigger)
  )
}
export function mouseTriggerFromButton(button: number): string | null {
  const names = ['MouseLeft', 'MouseMiddle', 'MouseRight', 'MouseBack', 'MouseForward']
  return Number.isInteger(button) && button >= 0 && button < 32
    ? names[button] || `MouseButton${button + 1}`
    : null
}
export function captureTriggerLabel(trigger: string) {
  return (
    mouseTriggers.find((item) => item.value === trigger)?.label ||
    (isMouseTrigger(trigger) ? `鼠标按键 ${trigger.slice(11)}` : trigger)
  )
}
