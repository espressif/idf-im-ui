<template>
  <transition name="slide-up">
    <div
      v-if="showUpdateRibbon"
      class="update-ribbon"
      data-id="update-ribbon"
      @click="handleRibbonClick"
    >
      <div class="update-content">
        <div class="update-icon">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
          </svg>
        </div>
        <div class="update-text">
          <span class="update-title">{{ t('app.updateAvailable.title') }}</span>
          <span class="update-version">{{ latestVersion }}</span>
        </div>
        <button class="update-close" @click.stop="dismissUpdate" data-id="dismiss-update">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
          </svg>
        </button>
      </div>
    </div>
  </transition>

  <!-- Update Details Modal -->
  <n-modal
    v-model:show="showUpdateModal"
    preset="card"
    :title="t('app.updateAvailable.modalTitle')"
    style="width: 600px"
    :bordered="false"
    data-id="update-modal"
  >
    <div class="update-modal-content">
      <div class="version-info">
        <div class="version-item">
          <span class="version-label">{{ t('app.updateAvailable.currentVersion') }}:</span>
          <span class="version-value current">{{ currentVersion }}</span>
        </div>
        <div class="version-arrow">→</div>
        <div class="version-item">
          <span class="version-label">{{ t('app.updateAvailable.latestVersion') }}:</span>
          <span class="version-value latest">{{ latestVersion }}</span>
        </div>
      </div>

      <div class="update-instructions">
        <h3>{{ t('app.updateAvailable.howToUpdate') }}</h3>

        <!-- OS-specific instructions -->
        <div class="instruction-section" v-if="updateCommand">
          <p class="instruction-desc">{{ updateDescription }}</p>
          <div class="command-box" data-id="update-command">
            <code>{{ updateCommand }}</code>
            <button
              class="copy-btn"
              @click="copyCommand"
              :title="t('app.updateAvailable.copyCommand')"
              data-id="copy-command-btn"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/>
              </svg>
            </button>
          </div>
        </div>

        <!-- Manual download option -->
        <div class="instruction-section">
          <p class="instruction-desc">
            {{ t('app.updateAvailable.orDownload') }}
          </p>
          <n-button
            type="error"
            size="large"
            @click="openDownloadPage"
            data-id="download-update-btn"
            block
          >
            {{ t('app.updateAvailable.downloadButton') }}
          </n-button>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="modal-footer">
        <n-button @click="dismissAndClose" data-id="dismiss-modal-btn">
          {{ t('app.updateAvailable.dismissButton') }}
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<script>
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'
import { NModal, NButton, useMessage } from 'naive-ui'

export default {
  name: 'UpdateNotification',
  components: {
    NModal,
    NButton,
  },
  setup() {
    const { t } = useI18n()
    const message = useMessage()

    const showUpdateRibbon = ref(false)
    const showUpdateModal = ref(false)
    const currentVersion = ref('0.0.0')
    const latestVersion = ref('0.0.0')
    const currentPlatform = ref('unknown')

    const DOWNLOAD_URL = 'https://dl.espressif.com/dl/eim/index.html'

    // Parse version string to comparable numbers
    const parseVersion = (versionStr) => {
      const cleaned = versionStr.replace(/^v/i, '')
      const parts = cleaned.split('.').map(Number)
      return {
        major: parts[0] || 0,
        minor: parts[1] || 0,
        patch: parts[2] || 0
      }
    }

    // Compare versions
    const isNewerVersion = (current, latest) => {
      const curr = parseVersion(current)
      const lat = parseVersion(latest)

      if (lat.major > curr.major) return true
      if (lat.major < curr.major) return false
      if (lat.minor > curr.minor) return true
      if (lat.minor < curr.minor) return false
      return lat.patch > curr.patch
    }

    // Fetch and parse the latest version from the website
    const fetchLatestVersion = async () => {
    try {
      const data = await invoke('fetch_json_from_url', {
        url: 'https://dl.espressif.com/dl/eim/eim_unified_release.json'
      })
      const version = data.tag_name
      console.log('Fetched latest version:', version)

      if (version) {
        return version
      }

      return null
    } catch (error) {
      console.log('Failed to fetch latest version:', error)
      return null
    }
  }



    // Dismiss update
    const dismissUpdate = () => {
      showUpdateRibbon.value = false
    }

    const dismissAndClose = () => {
      dismissUpdate()
      showUpdateModal.value = false
    }

    // Get OS-specific update instructions
    const updateCommand = computed(() => {
      const os = currentPlatform.value.toLowerCase()

      if (os.includes('linux')) {
        return 'sudo apt update && sudo apt upgrade eim'
      } else if (os.includes('darwin') || os.includes('macos')) {
        return 'brew update && brew upgrade eim'
      }
      return null // Windows doesn't have command-line update
    })

    const updateDescription = computed(() => {
      const os = currentPlatform.value.toLowerCase()

      if (os.includes('linux')) {
        return t('app.updateAvailable.updateWithApt')
      } else if (os.includes('darwin') || os.includes('macos')) {
        return t('app.updateAvailable.updateWithBrew')
      }
      return t('app.updateAvailable.downloadManually')
    })

    // Check for updates
    const checkForUpdates = async () => {
      try {
        // Get current version and platform
        currentVersion.value = 'v' + await getVersion()
        console.log('Current version:', currentVersion.value)
        currentPlatform.value = await invoke('get_operating_system')

        // Fetch latest version
        const latest = await fetchLatestVersion()
        console.log('Latest version:', latest)
        if (!latest) {
          return // Failed to fetch, fail silently
        }

        latestVersion.value = latest

        // Check if update is available and not dismissed
        if (isNewerVersion(currentVersion.value, latest)) {
          showUpdateRibbon.value = true
        }
      } catch (error) {
        // Fail silently
        console.log('Update check failed:', error)
      }
    }

    const handleRibbonClick = () => {
      showUpdateModal.value = true
    }

    const copyCommand = async () => {
      if (updateCommand.value) {
        try {
          await navigator.clipboard.writeText(updateCommand.value)
          message.success(t('app.updateAvailable.commandCopied'))
        } catch (error) {
          message.error(t('app.updateAvailable.copyFailed'))
        }
      }
    }

    const openDownloadPage = async () => {
      try {
        await open(DOWNLOAD_URL)
      } catch (error) {
        console.error('Failed to open download page:', error)
      }
    }

    onMounted(() => {
      // Check for updates on mount
      checkForUpdates()
    })

    return {
      t,
      showUpdateRibbon,
      showUpdateModal,
      currentVersion,
      latestVersion,
      updateCommand,
      updateDescription,
      handleRibbonClick,
      dismissUpdate,
      dismissAndClose,
      copyCommand,
      openDownloadPage,
    }
  }
}
</script>

<style scoped>
/* Update Ribbon */
.update-ribbon {
  position: fixed;
  bottom: 80px;
  right: 20px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  padding: 1rem 1.5rem;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15), 0 0 0 1px rgba(255, 255, 255, 0.1);
  cursor: pointer;
  z-index: 1000;
  max-width: 320px;
  transition: all 0.3s ease;
}

.update-ribbon:hover {
  transform: translateY(-2px);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.2), 0 0 0 1px rgba(255, 255, 255, 0.1);
}

.update-content {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.update-icon {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.update-icon svg {
  width: 18px;
  height: 18px;
}

.update-text {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.update-title {
  font-weight: 600;
  font-size: 0.95rem;
}

.update-version {
  font-size: 0.85rem;
  opacity: 0.9;
}

.update-close {
  flex-shrink: 0;
  background: transparent;
  border: none;
  color: white;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.2s ease;
  padding: 0;
}

.update-close:hover {
  background: rgba(255, 255, 255, 0.2);
}

.update-close svg {
  width: 18px;
  height: 18px;
}

/* Slide up animation */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.3s ease;
}

.slide-up-enter-from {
  transform: translateY(100px);
  opacity: 0;
}

.slide-up-leave-to {
  transform: translateY(100px);
  opacity: 0;
}

/* Modal Content */
.update-modal-content {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.version-info {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1.5rem;
  padding: 1.5rem;
  background: #f9fafb;
  border-radius: 8px;
}

.version-item {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  text-align: center;
}

.version-label {
  font-size: 0.875rem;
  color: #6b7280;
  font-weight: 500;
}

.version-value {
  font-size: 1.25rem;
  font-weight: 700;
  padding: 0.5rem 1rem;
  border-radius: 6px;
}

.version-value.current {
  color: #6b7280;
  background: #e5e7eb;
}

.version-value.latest {
  color: #059669;
  background: #d1fae5;
}

.version-arrow {
  font-size: 1.5rem;
  color: #6b7280;
}

.update-instructions {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.update-instructions h3 {
  font-size: 1.125rem;
  font-weight: 600;
  color: #374151;
  margin: 0;
}

.instruction-section {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.instruction-desc {
  color: #6b7280;
  font-size: 0.9375rem;
  margin: 0;
}

.command-box {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 1rem;
  background: #1f2937;
  border-radius: 6px;
  border: 1px solid #374151;
}

.command-box code {
  flex: 1;
  color: #10b981;
  font-family: 'Courier New', monospace;
  font-size: 0.875rem;
  overflow-x: auto;
}

.copy-btn {
  flex-shrink: 0;
  background: #374151;
  border: none;
  color: #9ca3af;
  width: 32px;
  height: 32px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 0;
}

.copy-btn:hover {
  background: #4b5563;
  color: white;
}

.copy-btn svg {
  width: 16px;
  height: 16px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
}
</style>
