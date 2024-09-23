import { createRouter, createWebHashHistory } from 'vue-router'
import WizardStep from './components/WizardStep.vue'

const routes = [
  { path: '/:step', component: WizardStep, props: true },
  { path: '/', redirect: '/1' }
]

const router = createRouter({
  history: createWebHashHistory(),
  routes
})

export default router