import { createRouter, createWebHashHistory } from 'vue-router'

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/capture',
      component: () => import('../pages/QuickCapture.vue'),
      meta: { title: '快速添加' },
    },
    { path: '/', component: () => import('../pages/DashboardPage.vue'), meta: { title: '概览' } },
    {
      path: '/inbox',
      redirect: (to) => ({ path: '/tasks', query: { ...to.query, project: 'none' } }),
    },
    {
      path: '/today',
      redirect: (to) => ({ path: '/tasks', query: { ...to.query, date: 'today' } }),
    },
    {
      path: '/tasks',
      component: () => import('../pages/TasksPage.vue'),
      meta: { title: '待办', filter: 'all' },
    },
    {
      path: '/projects',
      component: () => import('../pages/ProjectsPage.vue'),
      meta: { title: '项目空间' },
    },
    {
      path: '/settings',
      component: () => import('../pages/SettingsPage.vue'),
      meta: { title: '设置' },
    },
    {
      path: '/trash',
      component: () => import('../pages/TasksPage.vue'),
      meta: { title: '回收站', filter: 'trash' },
    },
    {
      path: '/reminders',
      component: () => import('../pages/RemindersPage.vue'),
      meta: { title: '提醒中心' },
    },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
  scrollBehavior: () => ({ top: 0 }),
})
