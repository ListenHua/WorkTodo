<script lang="ts">
import { cloneVNode, defineComponent, h, onBeforeUnmount, ref, useId, watch } from 'vue'
import { NPopover } from 'naive-ui'

// Keep the original trigger element so layout, native refs and menu events are preserved.
export default defineComponent({
  name: 'IconHint',
  inheritAttrs: false,
  props: {
    text: { type: String, required: true },
    disabled: Boolean,
  },
  setup(props, { attrs, slots }) {
    const id = `icon-hint-${useId()}`
    const hovered = ref(false)
    const focused = ref(false)
    const hide = () => {
      hovered.value = false
      focused.value = false
    }
    // Hide hints on resize instead of leaving them at a trigger's previous position.
    watch(
      () => hovered.value || focused.value,
      (visible) => {
        if (visible) window.addEventListener('resize', hide)
        else window.removeEventListener('resize', hide)
      },
    )
    onBeforeUnmount(() => window.removeEventListener('resize', hide))
    function call(handler: unknown, event: Event) {
      if (Array.isArray(handler)) handler.forEach((item) => call(item, event))
      else if (typeof handler === 'function') handler(event)
    }
    return () =>
      h(
        NPopover,
        {
          trigger: 'hover',
          show: !props.disabled && (hovered.value || focused.value),
          disabled: props.disabled,
          delay: 250,
          duration: 100,
          animated: false,
          placement: 'top',
          showArrow: false,
          style: { pointerEvents: 'none' },
          'onUpdate:show': (show: boolean) => (hovered.value = show),
        },
        {
          trigger: () => {
            const original = slots.default?.()[0]
            if (!original) return null
            const trigger = cloneVNode(original, attrs, true)
            const originalProps = trigger.props || {}
            trigger.props = { ...originalProps }
            trigger.props['aria-describedby'] =
              !props.disabled && (hovered.value || focused.value)
                ? [originalProps['aria-describedby'], id].filter(Boolean).join(' ')
                : originalProps['aria-describedby']
            for (const name of ['onFocus', 'onBlur', 'onClick', 'onKeydown']) {
              trigger.props[name] = (event: Event) => {
                call(originalProps[name], event)
                if (name === 'onFocus') focused.value = true
                else if (name !== 'onKeydown' || (event as KeyboardEvent).key === 'Escape') hide()
              }
            }
            return trigger
          },
          default: () => h('span', { id, role: 'tooltip', class: 'icon-hint-content' }, props.text),
        },
      )
  },
})
</script>
