import { ref, shallowRef } from 'vue'

// Keep content available while the modal leaves; release it after the transition.
export function useModalState<T>(initialValue: () => T) {
  const show = ref(false)
  const data = shallowRef<T>(initialValue())

  function open(value: T) {
    data.value = value
    show.value = true
  }
  function close() {
    show.value = false
  }
  function afterLeave() {
    if (!show.value) data.value = initialValue()
  }

  return { show, data, open, close, afterLeave }
}
