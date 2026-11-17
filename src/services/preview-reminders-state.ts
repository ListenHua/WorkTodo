import type { Reminder } from './desktop'
export const previewReminders: Reminder[] = []
export function cancelPreviewReminders(taskId: string) {
  for (const reminder of previewReminders) {
    if (reminder.taskId === taskId && reminder.state === 'pending') reminder.state = 'dismissed'
  }
}
