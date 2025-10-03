import { createRouter, createWebHashHistory } from "vue-router";
import Welcome from "./components/Welcome.vue";
import VersionManagement from "./components/VersionManagement.vue";
import BasicInstaller from "./components/BasicInstaller.vue";
import OfflineInstaller from "./components/OfflineInstaller.vue";
import SimpleSetup from "./components/SimpleSetup.vue";
import InstallationProgress from "./components/wizard_steps/InstalationProgress.vue";
import SimpleInstallatioProgressWrapper from "./components/SimpleInstallatioProgressWrapper.vue";
import WizardStep from "./components/WizardStep.vue";

const routes = [
  {
    path: "/",
    redirect: "/welcome",
    meta: { title: "routes.welcome" },
  },
  {
    path: "/welcome",
    name: "Welcome",
    component: Welcome,
    props: true,
    meta: { title: "routes.welcome" },
  },
  {
    path: "/version-management",
    name: "VersionManagement",
    component: VersionManagement,
    props: true,
    meta: { title: "routes.versionManagement" },
  },
  {
    path: "/basic-installer",
    name: "BasicInstaller",
    component: BasicInstaller,
    props: true,
    meta: { title: "routes.installationOptions" },
  },
  {
    path: "/offline-installer",
    name: "OfflineInstaller",
    component: OfflineInstaller,
    props: true,
    meta: { title: "routes.offlineInstallation" },
  },
  {
    path: "/simple-setup",
    name: "SimpleSetup",
    component: SimpleSetup,
    props: true,
    meta: { title: "routes.easyInstallation" },
  },
  {
    path: "/installation-progress",
    name: "InstallationProgress",
    component: SimpleInstallatioProgressWrapper,
    props: true,
    meta: { title: "routes.installationProgress" },
  },
  {
    path: "/wizard/:step",
    name: "Wizard",
    component: WizardStep,
    props: true,
    meta: { title: "routes.configurationWizard" },
  },
  {
    path: "/:pathMatch(.*)*",
    redirect: "/welcome",
    meta: { title: "routes.fallbackRedirect" },
  },
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
