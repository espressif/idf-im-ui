<template>
  <div class="welcome-container">
    <!-- Splash Screen -->
    <transition name="fade">
      <div v-if="showSplash" class="splash-screen">
        <div class="splash-content">
          <img src="../assets/espressif_logo.svg" alt="Espressif" class="logo" />
          <h1>ESP-IDF Installation Manager</h1>
          <p>Setting up your development environment...</p>
          <n-spin size="large" />
        </div>
      </div>
    </transition>

    <!-- Main Welcome Screen -->
    <main v-if="!showSplash" class="main-content">
      <!-- CPU Check for Windows -->
      <div class="welcome-card" v-if="os === 'windows' && cpuCount == 1">
        <h1>System Requirements</h1>
        <div class="content">
          <n-alert type="error" :bordered="false">
            This tool requires a system with at least 2 CPU cores when using Windows OS.
          </n-alert>
          <p>Sorry for the inconvenience</p>
          <n-button @click="quit" type="error" size="large">
            Exit Installer
          </n-button>
        </div>
      </div>

      <!-- Normal Welcome Flow -->
      <div class="welcome-card" v-else>
        <div class="welcome-header">
          <h1>Welcome to <span>ESP-IDF</span> Installation Manager</h1>
          <n-tag v-if="!isFirstRun" type="info">
            {{ installedVersionsCount }} version(s) installed
          </n-tag>
        </div>

        <div class="content">
          <p class="subtitle">{{ getWelcomeMessage }}</p>

          <!-- Quick Status -->
          <div v-if="checkingStatus" class="status-check">
            <n-spin size="small" />
            <span>Checking installation status...</span>
          </div>

          <!-- Decision Cards -->
          <div v-else class="decision-cards">
            <!-- Version Management (if versions exist) -->
            <n-card
              v-if="hasInstalledVersions"
              class="decision-card primary-action"
              @click="goToVersionManagement"
              hoverable
            >
              <div class="card-content">
                <n-icon :size="48" color="#E8362D">
                  <DashboardOutlined />
                </n-icon>
                <h3>Manage Installations</h3>
                <p>View and manage your {{ installedVersionsCount }} installed ESP-IDF version(s)</p>
                <n-button type="primary" block>Open Dashboard</n-button>
              </div>
            </n-card>

            <!-- Offline Installation (if archives detected) -->
            <n-card
              v-if="hasOfflineArchives"
              class="decision-card offline-action"
              @click="goToOfflineInstaller"
              hoverable
            >
              <div class="card-content">
                <n-icon :size="48" color="#52c41a">
                  <FileZipOutlined />
                </n-icon>
                <h3>Offline Installation</h3>
                <p>{{ offlineArchives.length }} archive(s) detected in current directory</p>
                <n-button type="success" block>Install from Archive</n-button>
              </div>
            </n-card>

            <!-- New Installation -->
            <n-card
              v-if="!hasInstalledVersions || showAllOptions"
              class="decision-card new-action"
              @click="goToBasicInstaller"
              hoverable
            >
              <div class="card-content">
                <n-icon :size="48" color="#1290d8">
                  <PlusCircleOutlined />
                </n-icon>
                <h3>New Installation</h3>
                <p>Install ESP-IDF development environment</p>
                <n-button type="info" block>Start Installation</n-button>
              </div>
            </n-card>
          </div>

          <!-- Show More Options -->
          <div v-if="hasInstalledVersions && !showAllOptions" class="more-options">
            <n-button @click="showAllOptions = true" block size="small">
              Show more options →
            </n-button>
          </div>

          <!-- Don't Show Again -->
          <div v-if="isFirstRun" class="preferences">
            <n-checkbox v-model:checked="dontShowAgain">
              Don't show this welcome screen again
            </n-checkbox>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>

<script>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import {
  NButton, NCard, NIcon, NTag, NSpin, NCheckbox, NAlert, useMessage
} from 'naive-ui'
import {
  DashboardOutlined,
  FileZipOutlined,
  PlusCircleOutlined
} from '@vicons/antd'

export default {
  name: 'Welcome',
  components: {
    NButton, NCard, NIcon, NTag, NSpin, NCheckbox, NAlert,
    DashboardOutlined, FileZipOutlined, PlusCircleOutlined
  },
  setup() {
    const router = useRouter()
    const message = useMessage()

    // System info
    const os = ref('unknown')
    const cpuCount = ref(0)

    // Installation status
    const showSplash = ref(true)
    const checkingStatus = ref(true)
    const hasInstalledVersions = ref(false)
    const installedVersionsCount = ref(0)
    const hasOfflineArchives = ref(false)
    const offlineArchives = ref([])

    // UI state
    const isFirstRun = ref(true)
    const dontShowAgain = ref(false)
    const showAllOptions = ref(false)

    const getWelcomeMessage = computed(() => {
      if (hasInstalledVersions.value && hasOfflineArchives.value) {
        return 'Choose how you want to proceed:'
      } else if (hasInstalledVersions.value) {
        return 'Manage your existing installations or install a new version:'
      } else if (hasOfflineArchives.value) {
        return 'Offline archives detected. You can install from local files or download online:'
      } else {
        return 'Get started with ESP-IDF development environment:'
      }
    })

    const checkSystem = async () => {
      try {
        os.value = await invoke('get_operating_system')
        cpuCount.value = await invoke('cpu_count')
      } catch (error) {
        console.error('Failed to check system:', error)
      }
    }

    const checkInstallationStatus = async () => {
      checkingStatus.value = true

      try {
        // Check for eim_idf.json and installed versions
        const versions = await invoke('get_installed_versions')
        hasInstalledVersions.value = versions && versions.length > 0
        installedVersionsCount.value = versions ? versions.length : 0

        // Check for .zst archives in current directory
        const archives = await invoke('scan_for_archives')
        hasOfflineArchives.value = archives && archives.length > 0
        offlineArchives.value = archives || []

        // Check if this is first run
        const settings = await invoke('get_app_settings')
        isFirstRun.value = settings?.first_run !== false

        // Auto-navigate based on status
        setTimeout(() => {
          autoNavigate()
        }, 500)

      } catch (error) {
        console.error('Failed to check installation status:', error)
      } finally {
        checkingStatus.value = false
      }
    }

    const autoNavigate = () => {
      // Don't auto-navigate if user disabled welcome screen
      if (!isFirstRun.value && dontShowAgain.value) {
        if (hasInstalledVersions.value) {
          router.replace('/version-management')
        } else if (hasOfflineArchives.value) {
          router.replace({
            path: '/offline-installer',
            query: { archives: JSON.stringify(offlineArchives.value) }
          })
        } else {
          router.replace('/basic-installer')
        }
      }
    }

    const savePreferences = async () => {
      if (dontShowAgain.value) {
        try {
          await invoke('save_app_settings', {
            settings: { first_run: false, skip_welcome: true }
          })
        } catch (error) {
          console.error('Failed to save preferences:', error)
        }
      }
    }

    const goToVersionManagement = async () => {
      await savePreferences()
      router.push('/version-management')
    }

    const goToOfflineInstaller = async () => {
      await savePreferences()
      router.push({
        path: '/offline-installer',
        query: { archives: JSON.stringify(offlineArchives.value) }
      })
    }

    const goToBasicInstaller = async () => {
      await savePreferences()
      router.push('/basic-installer')
    }

    const quit = async () => {
      try {
        await invoke('quit_app')
      } catch (error) {
        console.error('Failed to quit:', error)
      }
    }

    onMounted(async () => {
      await checkSystem()

      // Show splash for at least 1.5 seconds
      setTimeout(() => {
        showSplash.value = false
      }, 1500)

      await checkInstallationStatus()
    })

    return {
      os,
      cpuCount,
      showSplash,
      checkingStatus,
      hasInstalledVersions,
      installedVersionsCount,
      hasOfflineArchives,
      offlineArchives,
      isFirstRun,
      dontShowAgain,
      showAllOptions,
      getWelcomeMessage,
      goToVersionManagement,
      goToOfflineInstaller,
      goToBasicInstaller,
      quit
    }
  }
}
</script>

<style scoped>
.welcome-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #f5f5f5;
  background-image: url('../assets/bg.png');
  background-size: cover;
  background-position: center;
}

/* Splash Screen */
.splash-screen {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(135deg, #667eea 0%, #E8362D 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
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

/* Fade transition */
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.5s ease;
}

.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

/* Main Content */
.main-content {
  width: 100%;
  max-width: 1000px;
  padding: 2rem;
}

.welcome-card {
  background: white;
  padding: 3rem;
  border-radius: 12px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.1);
}

.welcome-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
}

.welcome-card h1 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 2.25rem;
  color: #1f2937;
  margin: 0;
}

.welcome-card h1 span {
  color: #E8362D;
}

.subtitle {
  font-family: 'Trueno-regular', sans-serif;
  font-size: 1.25rem;
  color: #4b5563;
  margin-bottom: 2rem;
}

.status-check {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 1rem;
  background: #f9fafb;
  border-radius: 8px;
  color: #6b7280;
}

/* Decision Cards */
.decision-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 1.5rem;
  margin: 2rem 0;
}

.decision-card {
  transition: all 0.3s ease;
  cursor: pointer;
  border: 2px solid transparent;
}

.decision-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
}

.decision-card.primary-action:hover {
  border-color: #E8362D;
}

.decision-card.offline-action:hover {
  border-color: #52c41a;
}

.decision-card.new-action:hover {
  border-color: #1290d8;
}

.card-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 1rem;
}

.card-content h3 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.25rem;
  color: #1f2937;
  margin: 0;
}

.card-content p {
  color: #6b7280;
  font-size: 0.95rem;
  margin: 0;
  min-height: 2.5rem;
}

.more-options {
  text-align: center;
  margin-top: 1rem;
  padding-left: 5rem;
  padding-right: 5rem;
}

.more-options .n-button {
  color: #e5e7eb;
}

.preferences {
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid #e5e7eb;
  text-align: center;
}

.n-button[type="primary"] {
  background-color: #E8362D;
}

.n-button[type="success"] {
  background-color: #52c41a;
}

.n-button[type="info"] {
  background-color: #1290d8;
}
</style>
