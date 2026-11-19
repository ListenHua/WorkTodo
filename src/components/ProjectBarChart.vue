<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useChartSize } from '../composables/useChartSize'
import { chartScale, type ProjectDistribution } from '../shared/dashboard'
import EmptyState from './EmptyState.vue'
import AppIcon from './AppIcon.vue'

const props = defineProps<{ projects: ProjectDistribution[]; loading: boolean }>()
const emit = defineEmits<{ select: [id: string]; browse: [] }>()
const { element, width, height } = useChartSize()
const page = ref(0)
const selectedId = ref<string | null>(null)
const pageSize = computed(() => (width.value < 250 ? 3 : width.value < 380 ? 4 : 6))
const pageCount = computed(() => Math.ceil(props.projects.length / pageSize.value))
watch(pageSize, () => {
  page.value = 0
})
watch(pageCount, (count) => {
  page.value = Math.min(page.value, Math.max(0, count - 1))
})
watch(page, () => {
  selectedId.value = null
})
const visible = computed(() =>
  props.projects.slice(page.value * pageSize.value, (page.value + 1) * pageSize.value),
)
const selected = computed(() => visible.value.find((p) => p.id === selectedId.value))
const scale = computed(() => chartScale(Math.max(0, ...visible.value.map((p) => p.total))))
const plot = computed(() => ({
  left: 34,
  top: 24,
  width: width.value - 48,
  height: height.value - 64,
}))
const slot = computed(() => plot.value.width / Math.max(1, visible.value.length))
const barWidth = computed(() => Math.min(42, slot.value * 0.5))
const y = (value: number) => plot.value.top + plot.value.height * (1 - value / scale.value.maximum)
const x = (index: number) => plot.value.left + slot.value * (index + 0.5)
const shortened = (name: string) =>
  [...name].length > 6 ? [...name].slice(0, 5).join('') + '…' : name
</script>

<template>
  <section class="panel dashboard-chart" aria-labelledby="project-chart-title" :aria-busy="loading">
    <div class="panel-heading dashboard-panel-heading">
      <div>
        <h2 id="project-chart-title">项目待办分布</h2>
      </div>
      <div class="chart-pagination" v-if="pageCount > 1">
        <span>{{ page + 1 }} / {{ pageCount }}</span>
        <button class="icon-button" aria-label="上一组项目" :disabled="page === 0" @click="page--">
          <AppIcon name="back" :size="16" />
        </button>
        <button
          class="icon-button"
          aria-label="下一组项目"
          :disabled="page + 1 >= pageCount"
          @click="page++"
        >
          <AppIcon name="arrow" :size="16" />
        </button>
      </div>
    </div>
    <div class="chart-legend">
      <span><i class="legend-completed" />待办总数</span>
    </div>
    <div ref="element" class="chart-plot project-bar-plot" @pointerleave="selectedId = null">
      <svg
        v-if="visible.length"
        :viewBox="`0 0 ${width} ${height}`"
        class="dashboard-svg"
        role="group"
        aria-label="项目待办柱状图"
      >
        <g aria-hidden="true" class="chart-axis">
          <g v-for="tick in scale.ticks" :key="tick">
            <line :x1="plot.left" :x2="width - 14" :y1="y(tick)" :y2="y(tick)" />
            <text :x="plot.left - 10" :y="y(tick) + 4" text-anchor="end">{{ tick }}</text>
          </g>
        </g>
        <g
          v-for="(project, index) in visible"
          :key="project.id"
          class="chart-bar-group"
          role="link"
          tabindex="0"
          :aria-label="`${project.name}，已完成 ${project.completed}，未完成 ${project.pending}，完成率 ${project.total ? Math.round((project.completed / project.total) * 100) : 0}%，查看项目`"
          @pointerenter="selectedId = project.id"
          @focus="selectedId = project.id"
          @blur="selectedId = null"
          @click="emit('select', project.id)"
          @keydown.enter.prevent="emit('select', project.id)"
          @keydown.space.prevent="emit('select', project.id)"
        >
          <rect
            class="chart-bar-hit"
            :x="x(index) - slot / 2 + 3"
            :y="plot.top - 12"
            :width="slot - 6"
            :height="plot.height + 52"
            rx="6"
          />
          <rect
            class="chart-bar"
            :x="x(index) - barWidth / 2"
            :y="y(project.total)"
            :width="barWidth"
            :height="(project.total / scale.maximum) * plot.height"
            rx="3"
          />
          <text
            class="chart-bar-total"
            :x="x(index)"
            :y="y(project.total) - 8"
            text-anchor="middle"
            aria-hidden="true"
          >
            {{ project.total }}
          </text>
          <text
            class="chart-project-label"
            :x="x(index)"
            :y="height - 14"
            text-anchor="middle"
            aria-hidden="true"
          >
            {{ shortened(project.name) }}
          </text>
        </g>
      </svg>
      <EmptyState
        v-else-if="!loading"
        kind="projects"
        title="暂无进行中的项目"
        action-label="点击查看项目"
        @action="emit('browse')"
      />
      <div v-else class="chart-loading" role="status">正在加载项目数据…</div>
      <div v-if="selected" class="chart-tooltip" role="tooltip">
        <strong>{{ selected.name }}</strong>
        <span
          >已完成 <b>{{ selected.completed }}</b></span
        ><span
          >未完成 <b>{{ selected.pending }}</b></span
        >
        <span
          >完成率
          <b
            >{{ selected.total ? Math.round((selected.completed / selected.total) * 100) : 0 }}%</b
          ></span
        >
      </div>
    </div>
  </section>
</template>
