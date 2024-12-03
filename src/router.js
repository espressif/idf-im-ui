import { createRouter, createWebHashHistory } from "vue-router";
import WizardStep from "./components/WizardStep.vue";
import Welcome from "./components/Welcome.vue";
import LoadConfig from "./components/LoadConfig.vue";
import SimpleSetup from "./components/SimpleSetup.vue";

const routes = [
  { path: "/welcome", component: Welcome, props: true },
  { path: "/load_config", component: LoadConfig, props: true },
  { path: "/simple-setup", component: SimpleSetup, props: true },

  { path: "/wizard/:step", component: WizardStep, props: true },
  { path: "/", redirect: "/welcome" },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
