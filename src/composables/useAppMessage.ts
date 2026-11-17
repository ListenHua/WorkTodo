import { h } from 'vue'
import { useMessage, type MessageApi } from 'naive-ui'
import AppIcon from '../components/AppIcon.vue'

export function useAppMessage() {
  const message = useMessage()
  const success: MessageApi['success'] = (content, options) =>
    message.success(content, {
      icon: () =>
        h(AppIcon, { name: 'success', size: 20, style: { color: 'var(--n-icon-color-success)' } }),
      ...options,
    })
  const error: MessageApi['error'] = (content, options) =>
    message.error(content, {
      icon: () =>
        h(AppIcon, { name: 'error', size: 20, style: { color: 'var(--n-icon-color-error)' } }),
      ...options,
    })
  const info: MessageApi['info'] = (content, options) =>
    message.info(content, {
      icon: () =>
        h(AppIcon, { name: 'info', size: 20, style: { color: 'var(--n-icon-color-info)' } }),
      ...options,
    })
  return { success, error, info }
}
