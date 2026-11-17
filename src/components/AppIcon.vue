<script setup lang="ts">
import type { Component } from 'vue'
import SunIcon from '@mingcute/vue/core-regular/sun'
import MoonIcon from '@mingcute/vue/core-regular/moon'
import GridIcon from '@mingcute/vue/core-regular/layout-grid'
import InboxIcon from '@mingcute/vue/core-regular/inbox'
import CalendarIcon from '@mingcute/vue/core-regular/calendar'
import CheckIcon from '@mingcute/vue/core-regular/check'
import ListIcon from '@mingcute/vue/core-regular/list-check'
import FolderIcon from '@mingcute/vue/core-regular/folder'
import FolderWarningIcon from '@mingcute/vue/core-regular/folder-warning'
import SettingsIcon from '@mingcute/vue/core-regular/settings-1'
import EditIcon from '@mingcute/vue/core-regular/edit-2'
import PlusIcon from '@mingcute/vue/core-regular/add'
import BackIcon from '@mingcute/vue/core-regular/arrow-left'
import ArrowIcon from '@mingcute/vue/core-regular/arrow-right'
import RefreshIcon from '@mingcute/vue/core-regular/refresh-1'
import MonitorIcon from '@mingcute/vue/core-regular/monitor'
import LeafIcon from '@mingcute/vue/core-regular/leaf'
import SearchIcon from '@mingcute/vue/core-regular/search'
import ClockIcon from '@mingcute/vue/core-regular/time'
import ExternalIcon from '@mingcute/vue/core-regular/external-link'
import InfoIcon from '@mingcute/vue/core-regular/information'
import CloseIcon from '@mingcute/vue/core-regular/close'
import ClearIcon from '@mingcute/vue/core-regular/close-circle'
import DownIcon from '@mingcute/vue/core-regular/down'
import BrandIcon from '@mingcute/vue/core-regular/folder-check'
import SuccessIcon from '@mingcute/vue/core-filled/check-circle'
import ErrorIcon from '@mingcute/vue/core-filled/close-circle'
import UploadIcon from '@mingcute/vue/core-regular/upload'
import ImageIcon from '@mingcute/vue/core-regular/pic'
import FileIcon from '@mingcute/vue/core-regular/file'
import MoreIcon from '@mingcute/vue/core-regular/more-1'
import DragIcon from '@mingcute/vue/core-regular/dots'
import DeleteIcon from '@mingcute/vue/core-regular/delete-2'
import EyeIcon from '@mingcute/vue/core-regular/eye'
import BookmarkIcon from '@mingcute/vue/core-regular/bookmark'
import BookmarkFilledIcon from '@mingcute/vue/core-filled/bookmark'

withDefaults(defineProps<{ name: string; size?: number; color?: string }>(), {
  size: 20,
  color: 'currentColor',
})

const icons: Record<string, Component> = {
  sun: SunIcon,
  moon: MoonIcon,
  grid: GridIcon,
  inbox: InboxIcon,
  calendar: CalendarIcon,
  check: CheckIcon,
  list: ListIcon,
  folder: FolderIcon,
  'folder-warning': FolderWarningIcon,
  settings: SettingsIcon,
  plus: PlusIcon,
  edit: EditIcon,
  arrow: ArrowIcon,
  back: BackIcon,
  refresh: RefreshIcon,
  monitor: MonitorIcon,
  leaf: LeafIcon,
  search: SearchIcon,
  clock: ClockIcon,
  external: ExternalIcon,
  info: InfoIcon,
  close: CloseIcon,
  clear: ClearIcon,
  down: DownIcon,
  brand: BrandIcon,
  success: SuccessIcon,
  error: ErrorIcon,
  upload: UploadIcon,
  image: ImageIcon,
  file: FileIcon,
  eye: EyeIcon,
  bookmark: BookmarkIcon,
  bookmarked: BookmarkFilledIcon,
  more: MoreIcon,
  drag: DragIcon,
  delete: DeleteIcon,
}
</script>

<template>
  <component
    :is="icons[name] || icons.folder"
    :size="size"
    :color="color"
    class="app-icon"
    aria-hidden="true"
    focusable="false"
  />
</template>
