<template>
  <n-config-provider :theme="theme">
    <n-message-provider>
      <n-dialog-provider>
        <n-notification-provider>
          <div id="app">
            <!-- Splash Screen -->
            <transition name="splash-fade">
              <div v-if="showSplash" class="splash-screen">
                <div class="splash-content">
                  <img src="./assets/espressif_logo.svg" alt="Espressif" class="logo" />
                  <h1>ESP-IDF Installation Manager</h1>
                  <p>Setting up your development environment...</p>
                  <n-spin size="large" />
                </div>
              </div>
            </transition>

            <!-- Header -->
            <header class="app-header" v-if="!showSplash">
              <div class="header-content">
                <div class="header-brand" @click="$router.push('/')" style="cursor: pointer">
                  <img src="./assets/espressif_logo_white.svg" alt="Espressif" class="logo" />
                  <span class="header-title">ESP-IDF Installation Manager</span>
                </div>
                <div class="header-actions">
                  <!-- Navigation breadcrumbs or status could go here -->
                  <!-- <n-breadcrumb v-if="showBreadcrumb">
                    <n-breadcrumb-item
                      v-for="crumb in breadcrumbs"
                      :key="crumb.path"
                      @click="goTo(crumb.path)"
                      style="cursor: pointer; color: rgb(230, 204, 204) !important"
                    >
                      <span style="color: rgb(230, 204, 204)">{{ crumb.label }}</span>
                    </n-breadcrumb-item>
                  </n-breadcrumb> -->
                </div>
              </div>
            </header>

            <!-- Main Content Area -->
            <main class="app-main">
              <router-view v-slot="{ Component }">
                <transition name="fade" mode="out-in">
                  <component :is="Component" />
                </transition>
              </router-view>
            </main>

            <!-- Footer -->
            <AppFooter v-if="!showSplash" />
          </div>
        </n-notification-provider>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script>
import { ref, computed, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NNotificationProvider,
  NBreadcrumb,
  NBreadcrumbItem,
  NSpin,
  darkTheme
} from 'naive-ui'
import AppFooter from './components/AppFooter.vue'
import { useRouter } from 'vue-router'

export default {
  name: 'App',
  components: {
    NConfigProvider,
    NMessageProvider,
    NDialogProvider,
    NNotificationProvider,
    NBreadcrumb,
    NBreadcrumbItem,
    NSpin,
    AppFooter
  },
  setup() {
    const route = useRoute()
    const theme = ref(null) // null for light theme, darkTheme for dark
    const showSplash = ref(true)
    const router = useRouter()

    // Hide splash screen after delay
    setTimeout(() => {
      showSplash.value = false
    }, 1500)

    // Breadcrumb configuration
    const routeToBreadcrumb = {
      '/welcome': { label: 'Welcome', show: false },
      '/version-management': { label: 'Version Management', show: true },
      '/basic-installer': { label: 'Installation Options', show: true },
      '/offline-installer': { label: 'Offline Installation', show: true },
      '/simple-setup': { label: 'Easy Installation', show: true },
      '/installation-progress': { label: 'Installation Progress', show: true },
      '/wizard': { label: 'Configuration Wizard', show: true }
    }

    const showBreadcrumb = computed(() => {
      const path = route.path
      for (const [key, value] of Object.entries(routeToBreadcrumb)) {
        if (path.startsWith(key)) {
          return value.show
        }
      }
      return false
    })

    const goTo = (path) => {
      router.push(path)
    }

    const breadcrumbs = computed(() => {
      const path = route.path
      const crumbs = [{ path: '/', label: 'Home' }]

      for (const [key, value] of Object.entries(routeToBreadcrumb)) {
        if (path.startsWith(key)) {
          crumbs.push({ path: key, label: value.label })
          break
        }
      }

      // Add wizard step if applicable
      if (path.startsWith('/wizard/')) {
        const step = route.params.step
        crumbs.push({ path: path, label: `Step ${step}` })
      }

      return crumbs
    })

    // Theme toggle (you can expose this through a settings menu)
    const toggleTheme = () => {
      theme.value = theme.value === null ? darkTheme : null
    }

    // Watch for system theme preference
    const checkSystemTheme = () => {
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
        // theme.value = darkTheme // Uncomment to respect system theme
      }
    }

    checkSystemTheme()
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', checkSystemTheme)

    return {
      theme,
      showBreadcrumb,
      breadcrumbs,
      goTo,
      toggleTheme,
      showSplash
    }
  }
}
</script>

<style>
/* Import base styles and fonts */
@import './assets/main.css';

/* Global Styles */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  height: 100%;
  overflow: hidden;
  margin: 0;
  padding: 0;
  min-height: 100vh;
  position: relative;
}

#app {
  height: 100vh;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: #f5f5f5;
  background-image: url('./assets/bg.png');
  background-size: cover;
  background-position: center;
  background-attachment: fixed;
  overflow: hidden;
  position: relative;
}

.app-main {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding-bottom: 60px;
  position: relative;
  height: calc(100vh - 60px - 60px);
  min-height: 0;
}

/* Header Styles */
.app-header {
  background: linear-gradient(135deg, #E8362D 0%, #dc2626 100%);
  color: white;
  padding: 1rem 1.5rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  z-index: 200;
}

.header-content {
  max-width: 1400px;
  margin: 0 auto;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-brand {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.header-brand .logo {
  height: 32px;
  width: auto;
}

.header-title {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.25rem;
  font-weight: 600;
}

.header-actions {
  display: flex;
  align-items: center;
}
/* Breadcrumb in header */
.app-header .n-breadcrumb {
  color: rgba(255, 255, 255, 0.8);
}

.app-header .n-breadcrumb-item__separator {
  color: rgba(255, 255, 255, 0.5);
}

/* Main Content Area */
.app-main {
  flex: 1;
  overflow-y: auto;
  padding-bottom: 60px; /* Space for footer */
  position: relative;
}

/* Page Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Splash Screen Transitions */
.splash-fade-enter-active {
  transition: opacity 0.5s ease;
}

.splash-fade-leave-active {
  transition: opacity 0.8s ease;
}

.splash-fade-enter-from,
.splash-fade-leave-to {
  opacity: 0;
}

/* Scrollbar Styles */
.app-main::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.app-main::-webkit-scrollbar-track {
  background: #f1f1f1;
}

.app-main::-webkit-scrollbar-thumb {
  background: #888;
  border-radius: 4px;
}

.app-main::-webkit-scrollbar-thumb:hover {
  background: #666;
}

/* Responsive Design */
@media (max-width: 768px) {
  .header-content {
    flex-direction: column;
    gap: 1rem;
  }

  .header-brand {
    justify-content: center;
  }

  .app-main {
    padding: 1rem;
  }
}

/* Loading State */
.loading-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(255, 255, 255, 0.9);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

/* Splash Screen */
.splash-screen {
  position: fixed;
  inset: 0;
  background: linear-gradient(135deg, #667eea 0%, #E8362D 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 99999;
}

.splash-content {
  text-align: center;
  color: white;
}

.splash-content .logo {
  width: 120px;
  height: auto;
  margin-bottom: 2rem;
  filter: brightness(0) invert(1);
}

.splash-content h1 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 2.5rem;
  margin-bottom: 1rem;
}

.splash-content p {
  font-size: 1.125rem;
  margin-bottom: 2rem;
  opacity: 0.9;
}

/* Utility Classes */
.text-center {
  text-align: center;
}

.mt-1 { margin-top: 0.25rem; }
.mt-2 { margin-top: 0.5rem; }
.mt-3 { margin-top: 0.75rem; }
.mt-4 { margin-top: 1rem; }

.mb-1 { margin-bottom: 0.25rem; }
.mb-2 { margin-bottom: 0.5rem; }
.mb-3 { margin-bottom: 0.75rem; }
.mb-4 { margin-bottom: 1rem; }

.p-1 { padding: 0.25rem; }
.p-2 { padding: 0.5rem; }
.p-3 { padding: 0.75rem; }
.p-4 { padding: 1rem; }

</style>
