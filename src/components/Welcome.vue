<template>
  <div class="welcome-container">
    <main class="main-content">
      <!-- CPU Check for Windows -->
      <div class="welcome-card" v-if="os === 'windows' && cpuCount == 1">
        <h1>{{ $t('welcome.systemRequirements') }}</h1>
        <div class="content">
          <n-alert type="error" :bordered="false">
            {{ $t('welcome.cpuError') }}
          </n-alert>
          <p>{{ $t('welcome.sorry') }}</p>
          <n-button @click="quit" type="error" size="large">
            {{ $t('welcome.exitInstaller') }}
          </n-button>
        </div>
      </div>

      <!-- Normal Welcome Flow -->
      <div class="welcome-card" v-else>
        <div class="welcome-header">
          <h1>{{$t('welcome.welcome')}} <span>ESP-IDF</span> {{$t('welcome.title')}}</h1>
        </div>

        <div class="content">
          <p class="subtitle">{{ getWelcomeMessage }}</p>

          <!-- Quick Status -->
          <div v-if="checkingStatus" class="status-check">
            <n-spin size="small" />
            <span>{{ $t('welcome.checkingStatus') }}</span>
          </div>

          <!-- Decision Cards -->
          <div v-else class="decision-cards">
            <!-- Version Management -->
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
                <h3>{{ $t('welcome.cards.manage.title') }}</h3>
                <p>{{ $t('welcome.cards.manage.description', { count: installedVersionsCount }) }}</p>
                <n-button type="primary" block>{{ $t('welcome.cards.manage.button') }}</n-button>
              </div>
            </n-card>

            <!-- Offline Installation -->
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
                <h3>{{ $t('welcome.cards.offline.title') }}</h3>
                <p>{{ $t('welcome.cards.offline.description', { count: offlineArchives.length }) }}</p>
                <n-button type="success" block>{{ $t('welcome.cards.offline.button') }}</n-button>
              </div>
            </n-card>

            <!-- New Installation -->
            <n-card class="decision-card new-action" @click="goToBasicInstaller" hoverable>
              <div class="card-content">
                <n-icon :size="48" color="#1290d8">
                  <PlusCircleOutlined />
                </n-icon>
                <h3>{{ $t('welcome.cards.new.title') }}</h3>
                <p>{{ $t('welcome.cards.new.description') }}</p>
                <n-button type="info" block>{{ $t('welcome.cards.new.button') }}</n-button>
              </div>
            </n-card>
          </div>

          <!-- Don't Show Again -->
          <div class="preferences">
            <n-checkbox v-model:checked="dontShowAgain">
              {{ $t('welcome.preferences.dontShow') }}
            </n-checkbox><br></br>
            <n-checkbox v-model:checked="allowUsageTracking" @update:checked="handleUsageTrackingChange">
              {{ $t('welcome.preferences.allowTracking') }}
            </n-checkbox><br></br>
            <a
              href="https://docs.espressif.com/projects/idf-im-ui/en/latest/#privacy-and-data-collection"
              target="_blank"
              rel="noopener noreferrer"
            >{{ $t('welcome.preferences.trackingDocs') }}</a>
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
import { useI18n } from 'vue-i18n'
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
    const { t } = useI18n()
    const router = useRouter()
    const message = useMessage()

    // System info
    const os = ref('unknown')
    const cpuCount = ref(0)

    // Installation status
    const checkingStatus = ref(true)
    const hasInstalledVersions = ref(false)
    const installedVersionsCount = ref(0)
    const hasOfflineArchives = ref(false)
    const offlineArchives = ref([])

    // UI state
    const isFirstRun = ref(true)
    const dontShowAgain = ref(false)
    const allowUsageTracking = ref(true)

    const getWelcomeMessage = computed(() => {
      if (hasInstalledVersions.value && hasOfflineArchives.value) {
        return t('welcome.messages.withBoth')
      } else if (hasInstalledVersions.value) {
        return t('welcome.messages.withInstalled')
      } else if (hasOfflineArchives.value) {
        return t('welcome.messages.withArchives')
      } else {
        return t('welcome.messages.fresh')
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
        console.log(`App settings: ${JSON.stringify(settings)}`)
        isFirstRun.value = settings?.first_run !== false
        dontShowAgain.value = settings?.skip_welcome === true

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
    const handleUsageTrackingChange = async (checked) => {
      if (checked) {
        message.success(t('welcome.messages.trackingEnabled'))
        try {
          await invoke('save_app_settings', {
            firstRun: false,
            skipWelcome: dontShowAgain.value,
            usageStatistics: checked
          })
        } catch (error) {
          console.error('Failed to change usage tracking settings:', error)
        }
      } else {
        try {
          await invoke('save_app_settings', {
            firstRun: false,
            skipWelcome: dontShowAgain.value,
            usageStatistics: checked
          })
        } catch (error) {
          console.error('Failed to change usage tracking settings:', error)
        }
        message.info(t('welcome.messages.trackingDisabled'))
      }
    }

    const savePreferences = async () => {
      if (dontShowAgain.value || allowUsageTracking.value === false) {
        try {
          await invoke('save_app_settings', {
            firstRun: false,
            skipWelcome: true,
            usageStatistics: allowUsageTracking.value
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
      await checkInstallationStatus()
    })

    return {
      os,
      cpuCount,
      checkingStatus,
      hasInstalledVersions,
      installedVersionsCount,
      hasOfflineArchives,
      offlineArchives,
      isFirstRun,
      dontShowAgain,
      getWelcomeMessage,
      allowUsageTracking,
      handleUsageTrackingChange,
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
  min-height: 100%;
  height: 100%;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #f5f5f5;
  background-image: url('../assets/bg.png');
  background-size: cover;
  background-position: center;
  position: relative;
  overflow: hidden;
  padding: 2rem 0;
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

.hidden {
  display: none;
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
  max-width: 1200px;
  padding: 0 2rem;
  margin: 0 auto;
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 100%;
}

.welcome-card {
  background: white;
  padding: 3rem 4rem;
  border-radius: 12px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.1);
  width: 100%;
  max-width: 1000px;
  margin: auto;
  position: relative;
}


.welcome-header {
  display: flex;
  justify-content: center;
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
  display: flex;
  gap: 2rem;
  margin: 2rem 0;
  justify-content: center;
  flex-wrap: wrap;
  width: 100%;
  padding: 0 1rem;
}

.decision-card {
  transition: all 0.3s ease;
  cursor: pointer;
  border: 2px solid transparent;
  flex: 1;
  min-width: 280px;
  max-width: 350px;
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

.app-main::-webkit-scrollbar {
  width: 8px;
  height: 8px;
  background: transparent;
}

.app-main::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.05);
  border-radius: 4px;
}

.app-main::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.app-main::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}

@media screen and (-webkit-min-device-pixel-ratio: 1) {
  .welcome-container {
    transform: translateZ(0); /* Force hardware acceleration */
  }

  .main-content {
    transform: translateZ(0); /* Force hardware acceleration */
  }
}

/* Fix for Windows high DPI displays */
@media screen and (min-resolution: 120dpi) {
  .welcome-card {
    border: 1px solid rgba(0, 0, 0, 0.05); /* Add subtle border for better definition */
  }
}

/* Responsive adjustments for Windows */
@media (max-width: 1100px) {
  .main-content {
    max-width: 95%;
    padding: 0 1rem;
  }

  .welcome-card {
    padding: 2rem;
    max-width: none;
  }

  .decision-cards {
    flex-direction: row;
    align-items: center;
    gap: 1rem;
  }

  .decision-card {
    max-width: 400px;
    min-width: 300px;
  }
}
</style>
