<template>
  <div class="version-management">
    <div class="management-header">
      <h1 class="title">{{ t('versionManagement.title') }}</h1>
      <div class="header-actions">
        <!-- <n-button @click="checkForUpdates" :loading="checkingUpdates" type="info" secondary class="check-updates">
          <template #icon>
            <n-icon><ReloadOutlined /></n-icon>
          </template>
          {{ t('versionManagement.checkForUpdates') }}
        </n-button> -->
      </div>
    </div>

    <!-- Prerequisites Alert (Windows) -->
    <n-alert
      v-if="os === 'windows' && !prerequisitesInstalled"
      type="warning"
      closable
      class="prerequisites-alert"
    >
      <template #header>{{ t('versionManagement.prerequisites.missing') }}</template>
      {{ t('versionManagement.prerequisites.windowsMessage') }}
      <n-button @click="installPrerequisites" size="small" type="warning" style="margin-left: 10px;">
        {{ t('versionManagement.prerequisites.installButton') }}
      </n-button>
    </n-alert>

    <!-- Installed Versions -->
    <div v-if="installedVersions.length > 0" class="versions-section">
      <h2>{{ t('versionManagement.sections.installedVersions') }}</h2>
      <div class="version-cards">
        <n-card
          v-for="version in installedVersions"
          :key="version.id"
          class="version-card"
          hoverable
        >
          <div class="version-card-content">
            <div class="version-info">
              <h3>{{ version.name }}</h3>
              <!-- <n-tag :type="version.active ? 'success' : 'default'" size="small">
                {{ version.version }}
              </n-tag> -->
            </div>
            <div class="version-path">
              <n-icon><FolderOutlined /></n-icon>
              <span>{{ version.path }}</span>
            </div>
            <!-- <div class="version-meta">
              <span class="install-date">Installed: {{ formatDate(version.installDate) }}</span>
              <span class="version-size">Size: {{ formatSize(version.size) }}</span>
            </div> -->
          </div>
          <div class="version-actions">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="openIDFTerminl(version)" quaternary circle>
                  <template #icon>
                    <n-icon><LaptopOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.openTerminal') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="renameVersion(version)" quaternary circle>
                  <template #icon>
                    <n-icon><EditOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.rename') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="fixVersion(version)" quaternary circle>
                  <template #icon>
                    <n-icon><ToolOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.fix') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="openInExplorer(version)" quaternary circle>
                  <template #icon>
                    <n-icon><FolderOpenOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.openFolder') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="removeVersion(version)" quaternary circle type="error">
                  <template #icon>
                    <n-icon><DeleteOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.remove') }}
            </n-tooltip>
          </div>
        </n-card>
      </div>
    </div>

    <!-- No Versions Installed -->
    <div v-else class="empty-state">
      <n-empty :description="t('versionManagement.sections.noVersions')">
        <template #icon>
          <n-icon :size="64" :depth="3">
            <FolderOpenOutlined />
          </n-icon>
        </template>
      </n-empty>
    </div>

    <!-- Quick Actions -->
    <div class="quick-actions">
      <n-button @click="goToBasicInstaller" type="primary" size="large">
        <template #icon>
          <n-icon><PlusCircleOutlined /></n-icon>
        </template>
        {{ t('versionManagement.quickActions.installNew') }}
      </n-button>

      <n-button
        v-if="os === 'windows'"
        @click="installDrivers"
        type="info"
        size="large"
      >
        <template #icon>
          <n-icon><UsbOutlined /></n-icon>
        </template>
        {{ t('versionManagement.quickActions.installDrivers') }}
      </n-button>

      <n-button
        v-if="installedVersions.length > 0"
        @click="purgeAll"
        type="error"
        size="large"
        secondary
        class="purge-all"
      >
        <template #icon>
          <n-icon><ClearOutlined /></n-icon>
        </template>
        {{ t('versionManagement.quickActions.purgeAll') }}
      </n-button>
    </div>

    <!-- Modals -->
    <n-modal
      v-model:show="showRenameModal"
      preset="dialog"
      :title="t('versionManagement.modals.rename.title')"
      :positive-text="t('versionManagement.modals.rename.confirmButton')"
      :negative-text="t('versionManagement.modals.rename.cancelButton')"
      :negative-button-props="{ textColor: '#e5e7eb' }"
      @positive-click="confirmRename"
    >
      <n-input
        v-model:value="newVersionName"
        :placeholder="t('versionManagement.modals.rename.placeholder')"
        @keyup.enter="confirmRename"
      />
    </n-modal>

    <n-modal
      v-model:show="showRemoveModal"
      preset="dialog"
      type="error"
      :title="t('versionManagement.modals.remove.title')"
      :positive-text="t('versionManagement.modals.remove.confirmButton')"
      :negative-text="t('versionManagement.modals.remove.cancelButton')"
      :negative-button-props="{ textColor: '#e5e7eb' }"
      @positive-click="confirmRemove"
    >
      <span v-html="t('versionManagement.modals.remove.message', { name: selectedVersion?.name })"></span>
      <br><br>
      {{ t('versionManagement.modals.remove.pathMessage') }}
      <br>
      <code>{{ selectedVersion?.path }}</code>
    </n-modal>

    <n-modal
      v-model:show="showFixModal"
      preset="dialog"
      type="error"
      :title="t('versionManagement.modals.fix.title')"
      :positive-text="t('versionManagement.modals.fix.confirmButton')"
      :negative-text="t('versionManagement.modals.fix.cancelButton')"
      :negative-button-props="{ textColor: '#e5e7eb' }"
      @positive-click="confirmFix"
    >
      <span v-html="t('versionManagement.modals.fix.message', { name: selectedVersion?.name })"></span>
      <br><br>
      {{ t('versionManagement.modals.fix.warning') }}
      <br>
      <code>{{ selectedVersion?.path }}</code>
    </n-modal>

    <n-modal
      v-model:show="showPurgeModal"
      preset="dialog"
      type="error"
      :title="t('versionManagement.modals.purge.title')"
      :positive-text="t('versionManagement.modals.purge.confirmButton')"
      :negative-text="t('versionManagement.modals.purge.cancelButton')"
      :negative-button-props="{ textColor: '#e5e7eb' }"
      @positive-click="confirmPurge"
    >
      <n-alert type="error" :bordered="false">
        {{ t('versionManagement.modals.purge.warning') }}
      </n-alert>
      <br>
      {{ t('versionManagement.modals.purge.listMessage') }}
      <ul>
        <li v-for="version in installedVersions" :key="version.id">
          {{ version.name }} ({{ version.path }})
        </li>
      </ul>
      <br>
      <n-checkbox v-model:checked="purgeConfirmed">
        {{ t('versionManagement.modals.purge.confirmation') }}
      </n-checkbox>
    </n-modal>
  </div>
</template>

<script>
import { ref, onMounted, version } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import {
  NButton, NCard, NIcon, NTag, NEmpty, NModal, NInput,
  NCheckbox, NAlert, NTooltip, useMessage
} from 'naive-ui'
import {
  FolderOutlined,
  FolderOpenOutlined,
  EditOutlined,
  DeleteOutlined,
  ToolOutlined,
  PlusCircleOutlined,
  ClearOutlined,
  ReloadOutlined,
  UsbOutlined,
  LaptopOutlined
} from '@vicons/antd'

export default {
  name: 'VersionManagement',
  components: {
    NButton, NCard, NIcon, NTag, NEmpty, NModal, NInput,
    NCheckbox, NAlert, NTooltip,
    FolderOutlined, FolderOpenOutlined, EditOutlined,
    DeleteOutlined, ToolOutlined, PlusCircleOutlined,
    ClearOutlined, ReloadOutlined, UsbOutlined, LaptopOutlined
  },
  setup() {
    const router = useRouter()
    const message = useMessage()
    const { t } = useI18n()

    const installedVersions = ref([])
    const os = ref('unknown')
    const prerequisitesInstalled = ref(true)
    const checkingUpdates = ref(false)

    // Modal states
    const showRenameModal = ref(false)
    const showRemoveModal = ref(false)
    const showFixModal = ref(false)
    const showPurgeModal = ref(false)
    const selectedVersion = ref(null)
    const newVersionName = ref('')
    const purgeConfirmed = ref(false)

    const loadInstalledVersions = async () => {
      try {
        const versions = await invoke('get_installed_versions')
        installedVersions.value = versions || []
      } catch (error) {
        console.error('Failed to load versions:', error)
        message.error(t('versionManagement.messages.error.loadVersions'))
      }
    }

    const checkOS = async () => {
      os.value = await invoke('get_operating_system')
      if (os.value === 'windows') {
        prerequisitesInstalled.value = await invoke('check_prerequisites')
      }
    }

    const formatDate = (dateString) => {
      return new Date(dateString).toLocaleDateString()
    }

    const formatSize = (bytes) => {
      const sizes = ['B', 'KB', 'MB', 'GB']
      if (bytes === 0) return '0 B'
      const i = Math.floor(Math.log(bytes) / Math.log(1024))
      return Math.round(bytes / Math.pow(1024, i) * 100) / 100 + ' ' + sizes[i]
    }

    const checkForUpdates = async () => {
      checkingUpdates.value = true
      try {
        const hasUpdate = await invoke('check_for_updates')
        if (hasUpdate) {
          message.info(t('versionManagement.messages.success.updateAvailable'))
        } else {
          message.success(t('versionManagement.messages.success.latestVersion'))
        }
      } catch (error) {
        message.error(t('versionManagement.messages.error.checkUpdates'))
      } finally {
        checkingUpdates.value = false
      }
    }

    const renameVersion = (version) => {
      selectedVersion.value = version
      newVersionName.value = version.name
      showRenameModal.value = true
    }

    const openIDFTerminl = async (version) => {
      try {
        let activationScript = version.activationScript;
        let res = await invoke('open_terminal_with_script', {
          scriptPath: activationScript,
        })
        if (!res) {
          message.error(t('versionManagement.messages.error.openTerminal'))
          return
        }
        console.log('IDF terminal opened successfully')
        message.success(t('versionManagement.messages.success.openTerminal'))
      } catch (error) {
        console.error("Terminal failed to open,", error)
        message.error(t('versionManagement.messages.error.openTerminal'))
      }
    }

    const confirmRename = async () => {
      console.log('Renaming installation:', selectedVersion.value.id, 'to', newVersionName.value);
      try {
        let res = await invoke('rename_installation', {
          id: selectedVersion.value.id,
          newName: newVersionName.value
        })
        if (!res) {
          message.error(t('versionManagement.messages.error.rename'))
          return
        }
        console.log('Installation renamed successfully')
        message.success(t('versionManagement.messages.success.renamed'))
        await loadInstalledVersions()
      } catch (error) {
        message.error(t('versionManagement.messages.error.rename'))
      }
    }

    const removeVersion = (version) => {
      selectedVersion.value = version
      showRemoveModal.value = true
    }

    const confirmRemove = async () => {
      try {
        let res = await invoke('remove_installation', {
          id: selectedVersion.value.id
        })
        if (res) {
          message.success(t('versionManagement.messages.success.removed'))
          await loadInstalledVersions()
        } else {
          message.error(t('versionManagement.messages.error.remove'))
        }
      } catch (error) {
        message.error(t('versionManagement.messages.error.remove'))
      }
    }

    const fixVersion = async (version) => {
      selectedVersion.value = version
      showFixModal.value = true
    }


    const confirmFix = async () => {
      try {
        // Start the fix process
        invoke('fix_installation', { id: selectedVersion.value.id })

        message.success(t('versionManagement.messages.success.repairStarted'))

        // Navigate to installation progress with fix mode parameters
        router.push({
          path: '/installation-progress',
          query: {
            mode: 'fix',
            id: selectedVersion.value.id,
            name: selectedVersion.value.name,
            path: selectedVersion.value.path,
            // Add auto-tracking flag so it knows repair is already in progress
            autotrack: 'true'
          }
        })
      } catch (error) {
        console.error('Fix installation error:', error)
        message.error(t('versionManagement.messages.error.repair', { error }))
      }
    }


    const openInExplorer = async (version) => {
      try {
        await invoke('show_in_folder', { path: version.path })
      } catch (error) {
        message.error(t('versionManagement.messages.error.openFolder'))
      }
    }

    const purgeAll = () => {
      purgeConfirmed.value = false
      showPurgeModal.value = true
    }

    const confirmPurge = async () => {
      if (!purgeConfirmed.value) {
        message.warning(t('versionManagement.messages.warning.confirmAction'))
        return
      }
      try {
        await invoke('purge_all_installations')
        message.success(t('versionManagement.messages.success.purged'))
        await loadInstalledVersions()
      } catch (error) {
        message.error(t('versionManagement.messages.error.purge', { error }))
      }
    }

    const installPrerequisites = async () => {
      try {
        await invoke('install_prerequisites')
        message.success(t('versionManagement.messages.success.prerequisitesStarted'))
      } catch (error) {
        message.error(t('versionManagement.messages.error.prerequisites', { error }))
      }
    }

    const installDrivers = async () => {
      try {
        let res = await invoke('check_elevation')
        if (!res) {
          message.error(t('versionManagement.messages.error.driversPermission'))
          return
        }
        await invoke('install_drivers').then(() => {
          message.success(t('versionManagement.messages.success.driversInstalled'))
        }).catch((error) => {
          message.error(t('versionManagement.messages.error.drivers', { error }))
        })
      } catch (error) {
        message.error(t('versionManagement.messages.error.drivers', { error }))
      }
    }

    const goToBasicInstaller = () => {
      router.push('/basic-installer')
    }

    onMounted(() => {
      checkOS()
      loadInstalledVersions()
    })

    return {
      installedVersions,
      os,
      prerequisitesInstalled,
      checkingUpdates,
      showRenameModal,
      showRemoveModal,
      showFixModal,
      showPurgeModal,
      selectedVersion,
      newVersionName,
      purgeConfirmed,
      formatDate,
      formatSize,
      checkForUpdates,
      renameVersion,
      openIDFTerminl,
      confirmRename,
      removeVersion,
      confirmRemove,
      confirmFix,
      fixVersion,
      openInExplorer,
      purgeAll,
      confirmPurge,
      installPrerequisites,
      installDrivers,
      goToBasicInstaller,
      t
    }
  }
}
</script>

<style scoped>
.version-management {
  padding: 2rem;
  max-width: 1400px;
  margin: 0 auto;
}

.management-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
}

.title {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 2rem;
  color: #1f2937;
  margin: 0;
}

.prerequisites-alert {
  margin-bottom: 2rem;
}

.versions-section h2 {
  font-family: 'Trueno-regular', sans-serif;
  font-size: 1.5rem;
  color: #374151;
  margin-bottom: 1.5rem;
}

.version-cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
  gap: 1.5rem;
  margin-bottom: 2rem;
}

.version-card {
  border: 1px solid #e5e7eb;
  transition: all 0.3s ease;
}

.version-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  transform: translateY(-2px);
}

.version-card-content {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.version-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.version-info h3 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.25rem;
  color: #1f2937;
  margin: 0;
}

.version-path {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: #6b7280;
  font-size: 0.875rem;
  padding: 0.5rem;
  background: #f9fafb;
  border-radius: 4px;
}

.version-meta {
  display: flex;
  justify-content: space-between;
  font-size: 0.875rem;
  color: #9ca3af;
}

.version-actions {
  display: flex;
  gap: 0.5rem;
  padding-top: 1rem;
  border-top: 1px solid #f3f4f6;
  color: #e5e7eb;
}

.version-actions .n-button {
  color: #e5e7eb;
}
.version-actions .n-button:hover {
  color: #1f2937;
}

.empty-state {
  padding: 4rem 2rem;
  text-align: center;
}

.quick-actions {
  display: flex;
  justify-content: center;
  gap: 1rem;
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid #e5e7eb;
}

.n-button[type="primary"] {
  background-color: #E8362D;
}

.n-modal code {
  background: #f3f4f6;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-size: 0.875rem;
}

.check-updates {
  color: #e5e7eb;
}
.purge-all {
  color: #e5e7eb;
}
.n-button {
  color: #e5e7eb;
}
.n-button__content {
  color: #e5e7eb;
}
</style>
