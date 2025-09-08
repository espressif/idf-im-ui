<template>
  <div class="offline-installer">
    <div class="installer-header">
      <h1 class="title">Offline Installation</h1>
      <n-button @click="goBack" type="primary" quaternary>
        <template #icon>
          <n-icon><ArrowLeftOutlined /></n-icon>
        </template>
        Back
      </n-button>
    </div>

    <!-- Archive Selection -->
    <n-card v-if="!installationStarted" class="config-card">
      <h2>Installation Configuration</h2>

      <!-- Selected Archives -->
      <div class="section">
        <h3>Selected Archive</h3>
        <div v-if="archives.length > 0" class="archive-list">
          <n-card v-for="(archive, index) in archives" :key="index" size="small">
            <div class="archive-item">
              <div class="archive-info">
                <n-icon size="24"><FileZipOutlined /></n-icon>
                <div>
                  <div class="archive-name">{{ getFileName(archive) }}</div>
                  <!-- <div class="archive-details">
                    Version: {{ extractVersion(archive) || 'Unknown' }}
                  </div> -->
                </div>
              </div>
              <n-button
                @click="removeArchive(index)"
                quaternary
                circle
                type="primary"
              >
                <template #icon>
                  <n-icon><CloseOutlined /></n-icon>
                </template>
              </n-button>
            </div>
          </n-card>
        </div>

        <n-button
          @click="addMoreArchives"
          dashed
          block
          style="margin-top: 1rem;"
          v-if="archives.length < 1"
        >
          <template #icon>
            <n-icon><PlusOutlined /></n-icon>
          </template>
          Add More Archives
        </n-button>
      </div>

      <!-- Installation Path -->
      <div class="section">
        <h3>Installation Path</h3>
        <n-input-group>
          <n-input
            v-model:value="installPath"
            placeholder="Select installation directory"
            :disabled="useDefaultPath"
          />
          <n-button @click="browsePath" :disabled="useDefaultPath">
            <template #icon>
              <n-icon><FolderOpenOutlined /></n-icon>
            </template>
            Browse
          </n-button>
        </n-input-group>
        <n-checkbox
          v-model:checked="useDefaultPath"
          style="margin-top: 0.5rem;"
        >
          Use default installation path
        </n-checkbox>
        <n-alert
          v-if="!pathValid && installPath"
          type="warning"
          style="margin-top: 1rem;"
        >
          The selected path is not empty. Installation may fail if files already exist.
        </n-alert>
      </div>

      <!-- Installation Options -->
      <!-- <div class="section">
        <h3>Options</h3>
        <n-space vertical>
          <n-checkbox v-model:checked="validateChecksum">
            Validate archive checksum
          </n-checkbox>
          <n-checkbox v-model:checked="createShortcuts">
            Create desktop shortcuts (Windows)
          </n-checkbox>
          <n-checkbox v-model:checked="addToPath">
            Add to system PATH
          </n-checkbox>
        </n-space>
      </div> -->

      <!-- Action Buttons -->
      <div class="actions">
        <n-button @click="goBack" size="large">
          Cancel
        </n-button>
        <n-button
          @click="startInstallation"
          type="primary"
          size="large"
          :disabled="archives.length === 0 || !installPath"
        >
          Start Installation
        </n-button>
      </div>
    </n-card>

    <!-- Installation Progress -->
    <n-card v-else class="progress-card">
      <h2>Installing ESP-IDF</h2>

      <div class="progress-section">
        <div class="current-status">
          <n-spin v-if="!installationComplete && !installationError" />
          <n-icon v-else-if="installationComplete" size="48" color="#52c41a">
            <CheckCircleOutlined />
          </n-icon>
          <n-icon v-else size="48" color="#ff4d4f">
            <CloseCircleOutlined />
          </n-icon>

          <h3>{{ currentStatus }}</h3>
          <p>{{ currentMessage }}</p>
        </div>

        <n-progress
          v-if="!installationComplete && !installationError"
          type="line"
          :percentage="progressPercentage"
          :indicator-placement="'inside'"
          :processing="true"
          color="#E8362D"
        />

        <!-- Installation Steps -->
        <n-steps
          v-if="!installationError"
          :current="currentStep"
          size="small"
          style="margin-top: 2rem;"
        >
          <n-step title="Extract Archive" />
          <n-step title="Verify Files" />
          <n-step title="Install Tools" />
          <n-step title="Configure Environment" />
          <n-step title="Complete" />
        </n-steps>
      </div>

      <!-- Installation Log -->
      <n-collapse
        v-if="installMessages.length > 0"
        style="margin-top: 2rem;"
      >
        <n-collapse-item title="Installation Log" name="1">
          <n-scrollbar style="max-height: 300px">
            <pre class="log-content">{{ installMessages.join('\n') }}</pre>
          </n-scrollbar>
        </n-collapse-item>
      </n-collapse>

      <!-- Completion Actions -->
      <div v-if="installationComplete || installationError" class="completion-actions">
        <n-button
          v-if="installationError"
          @click="retry"
          type="warning"
          size="large"
        >
          Retry Installation
        </n-button>
        <n-button
          @click="viewLogs"
          size="large"
        >
          View Full Logs
        </n-button>
        <n-button
          @click="finish"
          type="primary"
          size="large"
        >
          {{ installationComplete ? 'Finish' : 'Go Back' }}
        </n-button>
      </div>
    </n-card>
  </div>
</template>

<script>
import { ref, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event'
import {
  NButton, NCard, NIcon, NInput, NInputGroup, NCheckbox,
  NSpace, NAlert, NSpin, NProgress, NSteps, NStep,
  NCollapse, NCollapseItem, NScrollbar, useMessage
} from 'naive-ui'
import {
  ArrowLeftOutlined, FolderOpenOutlined, PlusOutlined,
  CloseOutlined, FileZipOutlined, CheckCircleOutlined,
  CloseCircleOutlined
} from '@vicons/antd'

export default {
  name: 'OfflineInstaller',
  components: {
    NButton, NCard, NIcon, NInput, NInputGroup, NCheckbox,
    NSpace, NAlert, NSpin, NProgress, NSteps, NStep,
    NCollapse, NCollapseItem, NScrollbar,
    ArrowLeftOutlined, FolderOpenOutlined, PlusOutlined,
    CloseOutlined, FileZipOutlined, CheckCircleOutlined,
    CloseCircleOutlined
  },
  setup() {
    const router = useRouter()
    const route = useRoute()
    const message = useMessage()

    // Configuration
    const archives = ref([])
    const installPath = ref('')
    const useDefaultPath = ref(true)
    const pathValid = ref(true)
    const validateChecksum = ref(true)
    const createShortcuts = ref(true)
    const addToPath = ref(true)

    // Installation state
    const installationStarted = ref(false)
    const installationComplete = ref(false)
    const installationError = ref(false)
    const currentStatus = ref('Preparing installation...')
    const currentMessage = ref('')
    const currentStep = ref(1)
    const progressPercentage = ref(0)
    const installMessages = ref([])

    let unlistenProgress = null
    let unlistenComplete = null

    const loadArchivesFromQuery = () => {
      if (route.query.archives) {
        console.log('Loading archives from query:', route.query.archives)
        try {
          archives.value = JSON.parse(route.query.archives)
        } catch (e) {
          console.error('Failed to parse archives:', e)
        }
      }
    }

    const getDefaultPath = async () => {
      try {
        const settings = await invoke('get_settings')
        installPath.value = settings?.path || ''
      } catch (error) {
        console.error('Failed to get default path:', error)
      }
    }

    const getFileName = (path) => {
      return path.split(/[/\\]/).pop()
    }

    const extractVersion = (path) => {
      const filename = getFileName(path)
      const match = filename.match(/esp-idf[_-]v?([\d.]+)/i)
      return match ? match[1] : null
    }

    const removeArchive = (index) => {
      archives.value.splice(index, 1)
    }

    const addMoreArchives = async () => {
      try {
        const selected = await open({
          multiple: true,
          filters: [{
            name: 'Archive Files',
            extensions: ['zst']
          }]
        })

        if (selected) {
          const newArchives = Array.isArray(selected) ? selected : [selected]
          archives.value.push(...newArchives)
        }
      } catch (error) {
        message.error('Failed to select archives')
      }
    }

    const browsePath = async () => {
      try {
        const selected = await open({
          directory: true,
          multiple: false
        })

        if (selected) {
          installPath.value = selected
          await validatePath()
        }
      } catch (error) {
        message.error('Failed to select path')
      }
    }

    const validatePath = async () => {
      try {
        pathValid.value = await invoke('is_path_empty_or_nonexistent_command', {
          path: installPath.value
        })
      } catch (error) {
        pathValid.value = false
      }
    }

    const startInstallation = async () => {
      installationStarted.value = true
      currentStep.value = 1

      // Set up event listeners
      unlistenProgress = await listen('offline-install-progress', (event) => {
        const { stage, message: msg, progress } = event.payload

        currentMessage.value = msg
        progressPercentage.value = progress || 0

        if (msg) {
          installMessages.value.push(msg)
        }

        // Update step based on stage
        switch(stage) {
          case 'extracting':
            currentStep.value = 1
            currentStatus.value = 'Extracting archive...'
            break
          case 'verifying':
            currentStep.value = 2
            currentStatus.value = 'Verifying files...'
            break
          case 'installing_tools':
            currentStep.value = 3
            currentStatus.value = 'Installing tools...'
            break
          case 'configuring':
            currentStep.value = 4
            currentStatus.value = 'Configuring environment...'
            break
          case 'complete':
            currentStep.value = 5
            currentStatus.value = 'Installation complete!'
            break
        }
      })

      unlistenComplete = await listen('installation_complete', (event) => {
        const { success, message: msg } = event.payload

        if (success) {
          installationComplete.value = true
          currentStatus.value = 'Installation Complete!'
          currentMessage.value = msg || 'ESP-IDF has been successfully installed.'
          progressPercentage.value = 100
        } else {
          installationError.value = true
          currentStatus.value = 'Installation Failed'
          currentMessage.value = msg || 'An error occurred during installation.'
        }
      })

      // Start installation
      try {
        await invoke('start_offline_installation', {
          archives: archives.value,
          installPath: useDefaultPath.value ? null : installPath.value,
          options: {
            validate_checksum: validateChecksum.value,
            create_shortcuts: createShortcuts.value,
            add_to_path: addToPath.value
          }
        })
      } catch (error) {
        installationError.value = true
        currentStatus.value = 'Installation Failed'
        currentMessage.value = error.toString()
      }
    }

    const retry = () => {
      installationStarted.value = false
      installationComplete.value = false
      installationError.value = false
      currentStep.value = 1
      progressPercentage.value = 0
      installMessages.value = []
    }

    const viewLogs = async () => {
      try {
        await invoke('open_logs_folder')
      } catch (error) {
        message.error('Failed to open logs folder')
      }
    }

    const finish = () => {
      if (installationComplete.value) {
        router.push('/version-management')
      } else {
        goBack()
      }
    }

    const goBack = () => {
      router.push('/basic-installer')
    }

    onMounted(() => {
      loadArchivesFromQuery()
      if (useDefaultPath.value) {
        getDefaultPath()
      }
    })

    return {
      archives,
      installPath,
      useDefaultPath,
      pathValid,
      validateChecksum,
      createShortcuts,
      addToPath,
      installationStarted,
      installationComplete,
      installationError,
      currentStatus,
      currentMessage,
      currentStep,
      progressPercentage,
      installMessages,
      getFileName,
      extractVersion,
      removeArchive,
      addMoreArchives,
      browsePath,
      validatePath,
      startInstallation,
      retry,
      viewLogs,
      finish,
      goBack
    }
  }
}
</script>

<style scoped>
.offline-installer {
  padding: 2rem;
  max-width: 900px;
  margin: 0 auto;
}

.installer-header {
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

.config-card, .progress-card {
  background: white;
  padding: 2rem;
}

.config-card h2, .progress-card h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.5rem;
  color: #374151;
  margin: 0 0 2rem 0;
}

.section {
  margin-bottom: 2rem;
}

.section h3 {
  font-family: 'Trueno-regular', sans-serif;
  font-size: 1.125rem;
  color: #4b5563;
  margin-bottom: 1rem;
}

.archive-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.archive-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.5rem;
}

.archive-info {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.archive-name {
  font-weight: 500;
  color: #1f2937;
}

.archive-details {
  font-size: 0.875rem;
  color: #6b7280;
  margin-top: 0.25rem;
}

.actions, .completion-actions {
  display: flex;
  justify-content: flex-end;
  gap: 1rem;
  padding-top: 2rem;
  border-top: 1px solid #e5e7eb;
}

.progress-section {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.current-status {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 2rem;
  background: #f9fafb;
  border-radius: 8px;
}

.current-status h3 {
  font-size: 1.25rem;
  color: #1f2937;
  margin: 1rem 0 0.5rem 0;
}

.current-status p {
  color: #6b7280;
  margin: 0;
}

.log-content {
  font-family: monospace;
  font-size: 0.875rem;
  line-height: 1.5;
  color: #374151;
  margin: 0;
}
.n-button {
  color: #e5e7eb;
}

.n-button[type="primary"] {
  color: #e5e7eb;
  background-color: #E8362D;
}
</style>
