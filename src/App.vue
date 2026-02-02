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
                  <h1>{{ $t('app.title') }}</h1>
                  <p>{{ $t('app.settingUp') }}</p>
                  <n-spin size="large" />
                </div>
              </div>
            </transition>

            <!-- Header -->
            <header class="app-header" v-if="!showSplash">
              <div class="header-content">
                <div class="header-brand" @click="$router.push('/')" style="cursor: pointer">
                  <img src="./assets/espressif_logo_white.svg" alt="Espressif" class="logo" />
                  <span class="header-title">{{ $t('app.title') }}</span>
                </div>
                <div class="header-actions">
                  <!-- Language Switcher -->
                  <n-dropdown
                    :options="languageOptions"
                    @select="handleLanguageChange"
                    trigger="click"
                  >
                    <n-button text style="color: white; font-size: 16px;">
                      <template #icon>
                        <n-icon>
                          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                            <path d="M12.87 15.07l-2.54-2.51.03-.03c1.74-1.94 2.98-4.17 3.71-6.53H17V4h-7V2H8v2H1v1.99h11.17C11.5 7.92 10.44 9.75 9 11.35 8.07 10.32 7.3 9.19 6.69 8h-2c.73 1.63 1.73 3.17 2.98 4.56l-5.09 5.02L4 19l5-5 3.11 3.11.76-2.04zM18.5 10h-2L12 22h2l1.12-3h4.75L21 22h2l-4.5-12zm-2.62 7l1.62-4.33L19.12 17h-3.24z"/>
                          </svg>
                        </n-icon>
                      </template>
                      {{ currentLanguageLabel }}
                    </n-button>
                  </n-dropdown>
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
            <UpdateNotification v-if="!showSplash" />
          </div>
        </n-notification-provider>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script>
import { ref, computed, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NNotificationProvider,
  NBreadcrumb,
  NBreadcrumbItem,
  NSpin,
  NButton,
  NDropdown,
  NIcon,
  darkTheme
} from 'naive-ui'
import AppFooter from './components/AppFooter.vue'
import UpdateNotification from './components/UpdateNotification.vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'

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
    NButton,
    NDropdown,
    NIcon,
    AppFooter,
    UpdateNotification
  },
  setup() {
    const route = useRoute()
    const router = useRouter()
    const { locale, t } = useI18n()
    const theme = ref(null)
    const showSplash = ref(true)

    // Hide splash screen after delay
    setTimeout(async () => {
      showSplash.value = false
      await invoke('set_locale', { locale: locale.value })
    }, 1500)

    // Language configuration
    const languages = [
      { key: 'en', label: 'English', flag: '🇺🇸' },
      { key: 'cn', label: '中文', flag: '🇨🇳' }
    ]

    // Load saved language preference or default to 'en'
    const savedLanguage = localStorage.getItem('app-language') || 'en'
    locale.value = savedLanguage

    const languageOptions = computed(() => {
      return languages.map(lang => ({
        label: `${lang.flag} ${lang.label}`,
        key: lang.key
      }))
    })

    const currentLanguageLabel = computed(() => {
      const current = languages.find(lang => lang.key === locale.value)
      return current ? `${current.flag} ${current.label}` : '🇺🇸 English'
    })

    const handleLanguageChange = async (key) => {
      locale.value = key
      localStorage.setItem('app-language', key)
      await invoke('set_locale', { locale: key }) // Notify backend of language change
    }

    // Breadcrumb configuration
    const routeToBreadcrumb = {
      '/welcome': { label: 'routes.welcome', show: false },
      '/version-management': { label: 'routes.versionManagement', show: true },
      '/basic-installer': { label: 'routes.installationOptions', show: true },
      '/offline-installer': { label: 'routes.offlineInstallation', show: true },
      '/simple-setup': { label: 'routes.easyInstallation', show: true },
      '/installation-progress': { label: 'routes.installationProgress', show: true },
      '/wizard': { label: 'routes.configurationWizard', show: true }
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
      const crumbs = [{ path: '/', label: 'app.home' }]

      for (const [key, value] of Object.entries(routeToBreadcrumb)) {
        if (path.startsWith(key)) {
          crumbs.push({ path: key, label: value.label })
          break
        }
      }

      if (path.startsWith('/wizard/')) {
        const step = route.params.step
        crumbs.push({
          path: path,
          label: 'routes.step',
          params: { n: step }
        })
      }

      return crumbs
    })

    const toggleTheme = () => {
      theme.value = theme.value === null ? darkTheme : null
    }

    const checkSystemTheme = () => {
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
        // theme.value = darkTheme
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
      showSplash,
      languageOptions,
      currentLanguageLabel,
      handleLanguageChange
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
  height: 100%;
  min-height: 100%;
  display: flex;
  flex-direction: column;
  background-color: #f5f5f5;
  background-image: url('./assets/bg.png');
  background-size: cover;
  background-position: center;
  background-attachment: fixed;
  overflow-y: auto;
  overflow-x: hidden;
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
  padding-bottom: 60px;
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
