import { createRouter, createWebHashHistory } from "vue-router";
import Welcome from "./components/Welcome.vue";
import VersionManagement from "./components/VersionManagement.vue";
import BasicInstaller from "./components/BasicInstaller.vue";
import OfflineInstaller from "./components/OfflineInstaller.vue";
import SimpleSetup from "./components/SimpleSetup.vue";
import InstallationProgress from "./components/wizard_steps/InstalationProgress.vue";
import WizardStep from "./components/WizardStep.vue";

const routes = [
  {
    path: "/",
    redirect: "/welcome"
  },
  {
    path: "/welcome",
    name: "Welcome",
    component: Welcome,
    props: true
  },
  {
    path: "/version-management",
    name: "VersionManagement",
    component: VersionManagement,
    props: true
  },
  {
    path: "/basic-installer",
    name: "BasicInstaller",
    component: BasicInstaller,
    props: true
  },
  {
    path: "/offline-installer",
    name: "OfflineInstaller",
    component: OfflineInstaller,
    props: true
  },
  {
    path: "/simple-setup",
    name: "SimpleSetup",
    component: SimpleSetup,
    props: true
  },
  {
    path: "/installation-progress",
    name: "InstallationProgress",
    component: InstallationProgress,
    props: true
  },
  {
    path: "/wizard/:step",
    name: "Wizard",
    component: WizardStep,
    props: true
  },
  // Fallback route
  {
    path: "/:pathMatch(.*)*",
    redirect: "/welcome"
  }
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

// Navigation guard to check prerequisites or handle navigation logic
router.beforeEach(async (to, from, next) => {
  // You can add logic here to check if the app should skip welcome screen
  // based on saved preferences
  next();
});

export default router;
