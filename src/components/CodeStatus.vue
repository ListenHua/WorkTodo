<script setup lang="ts">
import { gitLabel, type GitInfo } from '../shared/code-space'
defineProps<{ info: GitInfo | null; loading?: boolean; error?: string }>()
</script>
<template>
  <span
    class="code-status"
    :class="{ warning: info?.totalChanges, conflict: info?.conflicts || error }"
    role="status"
  >
    {{ loading ? '读取中…' : error ? '读取失败' : gitLabel(info) }}
  </span>
</template>
<style scoped>
.code-status {
  display: inline-flex;
  color: var(--muted);
  font-size: 12px;
  line-height: 24px;
}
.code-status.warning {
  color: #c16b15;
}
.code-status.conflict {
  color: #e24b4b;
}
:global(.dark) .code-status.warning {
  color: #eda255;
}
:global(.dark) .code-status.conflict {
  color: #ff8a8a;
}
</style>
