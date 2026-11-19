<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import EmptyState from './EmptyState.vue'
import { useChartSize } from '../composables/useChartSize'
import { chartScale, type TrendDay } from '../shared/dashboard'

const props = defineProps<{
  days: TrendDay[]
  today: string
  loading: boolean
  readOnly?: boolean
}>()
const emit = defineEmits<{ create: [] }>()
const { element, width, height } = useChartSize()
const active = ref<number | null>(null)
const todayIndex = computed(() =>
  Math.max(
    0,
    props.days.findIndex((day) => day.date === props.today),
  ),
)
watch(
  () => props.today,
  () => {
    active.value = null
  },
)
const hasActivity = computed(() => props.days.some((day) => day.created > 0 || day.completed > 0))
const scale = computed(() =>
  chartScale(Math.max(0, ...props.days.flatMap((d) => [d.created, d.completed]))),
)
const plot = computed(() => ({
  left: 34,
  top: 24,
  width: width.value - 52,
  height: height.value - 64,
}))
const x = (index: number) =>
  plot.value.left + (plot.value.width * index) / Math.max(1, props.days.length - 1)
const y = (value: number) => plot.value.top + plot.value.height * (1 - value / scale.value.maximum)
function path(series: 'created' | 'completed') {
  return props.days.map((day, i) => `${i ? 'L' : 'M'} ${x(i)} ${y(day[series])}`).join(' ')
}
const completedPath = computed(() => path('completed'))
const createdPath = computed(() => path('created'))
const area = computed(() =>
  props.days.length
    ? `${completedPath.value} L ${x(props.days.length - 1)} ${y(0)} L ${x(0)} ${y(0)} Z`
    : '',
)
const selected = computed(() => (active.value === null ? undefined : props.days[active.value]))
const totals = computed(() =>
  props.days.reduce(
    (sum, day) => ({
      created: sum.created + day.created,
      completed: sum.completed + day.completed,
    }),
    { created: 0, completed: 0 },
  ),
)
function point(event: PointerEvent) {
  if (!element.value) return
  const position = event.clientX - element.value.getBoundingClientRect().left
  active.value = Math.max(
    0,
    Math.min(
      props.days.length - 1,
      Math.round(((position - plot.value.left) / plot.value.width) * (props.days.length - 1)),
    ),
  )
}
function keyboard(event: KeyboardEvent) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End', 'Escape'].includes(event.key)) return
  event.preventDefault()
  if (event.key === 'Escape') active.value = null
  else if (event.key === 'Home') active.value = 0
  else if (event.key === 'End') active.value = props.days.length - 1
  else
    active.value = Math.max(
      0,
      Math.min(
        props.days.length - 1,
        (active.value ?? todayIndex.value) + (event.key === 'ArrowLeft' ? -1 : 1),
      ),
    )
}
</script>

<template>
  <section class="panel dashboard-chart" aria-labelledby="trend-chart-title" :aria-busy="loading">
    <div class="panel-heading dashboard-panel-heading">
      <div>
        <h2 id="trend-chart-title">待办趋势</h2>
      </div>
    </div>
    <div class="chart-legend">
      <span
        ><i class="legend-line" />已完成 <b>{{ totals.completed }}</b></span
      ><span
        ><i class="legend-line legend-created" />新增 <b>{{ totals.created }}</b></span
      >
    </div>
    <div
      ref="element"
      class="chart-plot trend-plot"
      role="group"
      tabindex="0"
      :aria-label="`以今天为中心的 7 天待办趋势，${days[0]?.date} 至 ${days[days.length - 1]?.date}，新增 ${totals.created} 项，完成 ${totals.completed} 项。使用左右方向键查看日期`"
      @pointermove="point"
      @pointerdown="point"
      @pointerleave="active = null"
      @focus="active = todayIndex"
      @blur="active = null"
      @keydown="keyboard"
    >
      <svg :viewBox="`0 0 ${width} ${height}`" class="dashboard-svg" aria-hidden="true">
        <g v-if="hasActivity" class="chart-axis">
          <g v-for="tick in scale.ticks" :key="tick">
            <line :x1="plot.left" :x2="width - 18" :y1="y(tick)" :y2="y(tick)" />
            <text :x="plot.left - 10" :y="y(tick) + 4" text-anchor="end">{{ tick }}</text>
          </g>
        </g>
        <path v-if="hasActivity" :d="area" class="chart-area" />
        <path v-if="hasActivity" :d="createdPath" class="chart-line chart-line-created" />
        <path v-if="hasActivity" :d="completedPath" class="chart-line chart-line-completed" />
        <g v-for="(day, index) in days" :key="day.date">
          <text
            class="chart-date-label"
            :class="{ 'is-today': day.date === today }"
            :x="x(index)"
            :y="height - 14"
            text-anchor="middle"
          >
            {{ day.date === today ? '今天' : day.label }}
          </text>
          <circle
            v-if="hasActivity"
            class="chart-trend-dot"
            :cx="x(index)"
            :cy="y(day.completed)"
            r="3"
          />
        </g>
        <g v-if="selected && active !== null">
          <line class="chart-crosshair" :x1="x(active)" :x2="x(active)" :y1="plot.top" :y2="y(0)" />
          <circle
            class="chart-selected-dot created-dot"
            :cx="x(active)"
            :cy="y(selected.created)"
            r="4"
          />
          <circle class="chart-selected-dot" :cx="x(active)" :cy="y(selected.completed)" r="4" />
        </g>
      </svg>
      <div v-if="selected" class="chart-tooltip" role="tooltip" aria-live="polite">
        <strong>{{ selected.date }}</strong><span
          >已完成 <b>{{ selected.completed }}</b></span
        ><span
          >新增 <b>{{ selected.created }}</b></span
        >
      </div>
      <div v-if="loading && !selected" class="chart-message" role="status">正在加载待办数据…</div>
      <EmptyState
        v-if="!loading && !hasActivity"
        class="trend-empty-state"
        kind="activity"
        title="暂无待办趋势"
        action-label="点击新增"
        :disabled="readOnly"
        @action="emit('create')"
      />
    </div>
  </section>
</template>
