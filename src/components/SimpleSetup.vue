<template>
  <div class="simple-setup">
    <div class="setup-header">
      <h1 class="title">{{ $t('simpleSetup.title') }}</h1>
      <n-button @click="goBack" quaternary v-if="currentState !== 'checking' && currentState !== 'installing'"
        text-color="white">
        <template #icon>
          <n-icon>
            <ArrowLeftOutlined />
          </n-icon>
        </template>
        {{ $t('simpleSetup.back') }}
      </n-button>
    </div>

    <!-- Pre-installation Check -->
    <n-card v-if="currentState === 'checking'" class="status-card">
      <div class="checking-status">
        <n-spin size="large" />
        <h2>{{ $t('simpleSetup.preparation.title') }}</h2>
        <p>{{ $t('simpleSetup.preparation.checking') }}</p>
      </div>
    </n-card>

    <!-- Ready to Install -->
    <n-card v-else-if="currentState === 'ready'" class="status-card">
      <div class="ready-status">
        <n-icon :size="64" color="#52c41a">
          <CheckCircleOutlined />
        </n-icon>
        <h2>{{ $t('simpleSetup.ready.title') }}</h2>
        <div class="installation-summary">
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.ready.summary.version') }}:</span>
            <span class="summary-value">{{ selectedVersion }}</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.ready.summary.path') }}:</span>
            <span class="summary-value">{{ installPath }}</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.ready.summary.size') }}:</span>
            <span class="summary-value">~3.5 GB</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.ready.summary.time') }}:</span>
            <span class="summary-value">10-45 minutes</span>
          </div>
        </div>
        <n-alert type="info" :bordered="false" style="margin: 1.5rem 0;">
          {{ $t('simpleSetup.ready.alert') }}
        </n-alert>
        <n-button @click="startInstallation" type="primary" size="large" block>
          {{ $t('simpleSetup.ready.startButton') }}
        </n-button>
      </div>
    </n-card>

    <!-- Installation Progress -->
    <n-card v-else-if="currentState === 'installing'" class="status-card">
      <div class="installing-status">
        <div class="status-header">
          <n-icon :size="48" :class="getStatusIconClass">
            <component :is="getStatusIcon" />
          </n-icon>
          <h2>{{ installationTitle }}</h2>
        </div>

        <p class="status-description">{{ installationMessage }}</p>

        <GlobalProgress :initial-message="installationMessage" :initial-progress="installationProgress"
          :show-details="true" :color-scheme="getProgressColorScheme" :steps="installationSteps"
          event-channel="installation-progress" />

        <n-collapse v-if="installMessages.length > 0" style="margin-top: 2rem;">
          <n-collapse-item :title="$t('simpleSetup.installation.log')" name="1">
            <n-scrollbar style="max-height: 180px">
              <pre class="log-content">{{ installMessages.join('\n') }}</pre>
            </n-scrollbar>
          </n-collapse-item>
        </n-collapse>
      </div>
    </n-card>

    <!-- Installation Complete -->
    <n-card v-else-if="currentState === 'complete'" class="status-card">
      <div class="complete-status">
        <n-result status="success" :title="$t('simpleSetup.complete.title')"
          :description="$t('simpleSetup.complete.description')">
          <template #icon>
            <n-icon :size="72" color="#52c41a">
              <CheckCircleOutlined />
            </n-icon>
          </template>
          <template #footer>
            <div class="completion-actions">
              <n-button @click="viewDocumentation" size="large">
                {{ $t('simpleSetup.complete.buttons.documentation') }}
              </n-button>
              <n-button @click="goToManagement" type="primary" size="large">
                {{ $t('simpleSetup.complete.buttons.dashboard') }}
              </n-button>
            </div>
          </template>
        </n-result>

        <div class="post-install-info" v-if="appStore.os == 'windows'">
          <h3>{{ $t('simpleSetup.complete.nextSteps.title') }}</h3>
          <ol>
            <li>{{ $t('simpleSetup.complete.nextSteps.step1') }}</li>
            <li>{{ $t('simpleSetup.complete.nextSteps.step2') }}</li>
            <li>{{ $t('simpleSetup.complete.nextSteps.step3', { command: 'idf.py create-project my_project' }) }}</li>
            <li>{{ $t('simpleSetup.complete.nextSteps.step4', { command: 'idf.py build' }) }}</li>
          </ol>
        </div>
        <div class="post-install-info" v-else>
          <h3>{{ $t('simpleSetup.complete.nextSteps_posix.title') }}</h3>
          <ol>
            <li>{{ $t('simpleSetup.complete.nextSteps_posix.step1') }}</li>
            <li>{{ $t('simpleSetup.complete.nextSteps_posix.step2') }}</li>
            <li>{{ $t('simpleSetup.complete.nextSteps_posix.step3', { command: `source ~/.espressif/tools/activate_idf_${selectedVersion}.sh` }) }}</li>
            <li>{{ $t('simpleSetup.complete.nextSteps_posix.step4', { command: 'idf.py build' }) }}</li>
          </ol>
        </div>
      </div>
    </n-card>

    <!-- Installation Failed -->
    <n-card v-else-if="currentState === 'error'" class="status-card">
      <div class="error-status">
        <n-result :status="verificationFailed ? 'warning' : 'error'" :title="errorTitle" :description="errorMessage">
          <template #icon>
            <n-icon :size="72" :color="verificationFailed ? '#faad14' : '#ff4d4f'">
              <CloseCircleOutlined />
            </n-icon>
          </template>
          <template #footer>
            <div class="error-actions">
              <n-button v-if="verificationFailed" @click="skipAndContinue" type="warning" size="large" data-id="skip-prerequisites-button">
                {{ $t('common.prerequisites.skipCheck') }}
              </n-button>
              <n-button @click="viewLogs" type="info" size="large">
                {{ $t('simpleSetup.error.viewLogs') }}
              </n-button>
              <n-button @click="retry" type="warning" size="large">
                {{ $t('simpleSetup.error.retry') }}
              </n-button>
              <n-button @click="useWizard" type="info" size="large">
                {{ $t('simpleSetup.error.wizard') }}
              </n-button>
            </div>
          </template>
        </n-result>

        <n-alert v-if="errorDetails" :type="verificationFailed ? 'warning' : 'error'" style="margin-top: 2rem;">
          <template #header>{{ $t('simpleSetup.error.details') }}</template>
          <pre class="error-details">{{ errorDetails }}</pre>
        </n-alert>
      </div>
    </n-card>
  </div>
</template>

<script>
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  NButton, NCard, NIcon, NSpin, NResult, NAlert,
  NCollapse, NCollapseItem, NScrollbar, useMessage
} from 'naive-ui'
import {
  ArrowLeftOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
  DownloadOutlined,
  ToolOutlined
} from '@vicons/antd'
import GlobalProgress from './GlobalProgress.vue'
import { useAppStore } from '../store'
import { openUrl } from '@tauri-apps/plugin-opener';
import { app } from '@tauri-apps/api'

export default {
  name: 'SimpleSetup',
  components: {
    NButton, NCard, NIcon, NSpin, NResult, NAlert,
    NCollapse, NCollapseItem, NScrollbar,
    ArrowLeftOutlined, CheckCircleOutlined, CloseCircleOutlined,
    LoadingOutlined, DownloadOutlined, ToolOutlined,
    GlobalProgress
  },
  setup() {
    const router = useRouter()
    const message = useMessage()
    const appStore = useAppStore()
    const { t } = useI18n()

    // State management
    const currentState = ref('checking') // checking, ready, installing, complete, error
    const selectedVersion = ref('latest')
    const installPath = ref('')
    const currentStep = ref(0)
    const timeStarted = ref(null)


    // Installation progress
    const installationTitle = ref('Installing ESP-IDF')
    const installationMessage = ref('Preparing installation...')
    const installationProgress = ref(0)
    const installMessages = ref([])
    const installationSteps = ref([
      {
        title: t('simpleSetup.installation.steps.check.title'),
        description: t('simpleSetup.installation.steps.check.description')
      },
      {
        title: t('simpleSetup.installation.steps.prerequisites.title'),
        description: t('simpleSetup.installation.steps.prerequisites.description')
      },
      { title: t('simpleSetup.installation.steps.download.title'), description: t('simpleSetup.installation.steps.download.description') },
      { title: t('simpleSetup.installation.steps.extract.title'), description: t('simpleSetup.installation.steps.extract.description') },
      { title: t('simpleSetup.installation.steps.tools.title'), description: t('simpleSetup.installation.steps.tools.description') },
      { title: t('simpleSetup.installation.steps.python.title'), description: t('simpleSetup.installation.steps.python.description') },
      { title: t('simpleSetup.installation.steps.configure.title'), description: t('simpleSetup.installation.steps.configure.description') },
      { title: t('simpleSetup.installation.steps.complete.title'), description: t('simpleSetup.installation.steps.complete.description') }
    ])

    // Error handling
    const errorTitle = ref('Installation Failed')
    const errorMessage = ref('')
    const errorDetails = ref('')
    const verificationFailed = ref(false)

    // Helper function to fetch settings, versions, and validate installation path
    const prepareForInstallation = async () => {
      try {
        const settings = await invoke('get_settings')
        installPath.value = settings?.path
        console.log('Default installation path:', installPath.value)

        const versions = await invoke('get_idf_versions', { includeUnstable: false })
        selectedVersion.value = versions?.[0]?.name || 'v5.5.1'
        console.log('Selected version:', selectedVersion.value)

        const pathValid = await invoke('is_path_empty_or_nonexistent_command', {
          path: installPath.value,
          versions: [selectedVersion.value]
        })
        console.log('Installation path check result:', pathValid)

        if (!pathValid) {
          errorTitle.value = 'Installation Path Not Empty'
          errorMessage.value = `The default installation path (${installPath.value}/${selectedVersion.value}) is not empty.`
          errorDetails.value = 'Please use custom installation to select a different path.'
          currentState.value = 'error'
          return false
        }

        currentState.value = 'ready'
        return true
      } catch (error) {
        console.error('Failed to prepare for installation:', error)
        errorTitle.value = t('simpleSetup.error.system.title')
        errorMessage.value = t('simpleSetup.error.system.message')
        errorDetails.value = error.toString()
        currentState.value = 'error'
        return false
      }
    }

    // Event listeners
    let unlistenProgress = null
    let unlistenComplete = null
    let unlistenError = null
    let unlistenLog = null

    const getStatusIcon = computed(() => {
      if (installationProgress.value < 30) return DownloadOutlined
      if (installationProgress.value < 70) return ToolOutlined
      if (installationProgress.value < 100) return LoadingOutlined
      return CheckCircleOutlined
    })

    const getStatusIconClass = computed(() => { // TODO
      return installationProgress.value < 100 ? 'rotating' : ''
    })

    const getProgressColorScheme = computed(() => {
      if (currentState.value === 'error') return 'error'
      if (installationProgress.value === 100) return 'success'
      return 'primary'
    })

    const checkPrerequisites = async (force) => {
      try {
        currentState.value = 'checking'
        verificationFailed.value = false
        if (appStore.prerequisitesLastChecked === null || force) {
          await appStore.checkPrerequisites(force);
        }
        // Check prerequisites
        let prereqResult = appStore.prerequisitesStatus;
        
        // Check if verification itself failed (shell or other error)
        if (!prereqResult.canVerify) {
          verificationFailed.value = true
          errorTitle.value = t('common.prerequisites.verificationFailedTitle')
          if (prereqResult.shellFailed) {
            errorMessage.value = t('common.prerequisites.shellFailed')
          } else {
            errorMessage.value = t('common.prerequisites.verificationError')
          }
          errorDetails.value = t('common.prerequisites.verificationFailedHint')
          currentState.value = 'error'
          return false
        }
        
        if (!prereqResult.allOk && appStore.os !== 'windows') {
          errorTitle.value = t('simpleSetup.error.prerequisites.title')
          errorMessage.value = t('simpleSetup.error.prerequisites.message')
          if (appStore.os === 'macos') {
            errorDetails.value = t('common.prerequisites.manualHint') + '\n' + t('common.prerequisites.macosHint', { list: prereqResult.missing.join(' ') })
          } else if (appStore.os === 'linux') {
            errorDetails.value = t('common.prerequisites.manualHint') + '\n' + t('common.prerequisites.linuxHint', { list: prereqResult.missing.join(' ') })
          } else {
            errorDetails.value = t('common.prerequisites.manualHint') + '\n' + prereqResult.missing.join(', ')
          }
          currentState.value = 'error'
          return false
        } // TODO: maybe on windows inform user which prerequisities will be installed
        let python_sane = await invoke("python_sanity_check", {});
        if (!python_sane) {
          if (appStore.os == 'windows') {
            console.log("Python sanity check failed - attempting automatic installation");
            try {
              console.log("Installing Python...");
              await invoke("python_install", {});
              python_sane = await invoke("python_sanity_check", {});
            } catch (error) {
              console.error('Automatic Python installation failed:', error);
              python_sane = false;
            }
          } else {
            console.log("Python sanity check failed");
            errorTitle.value = t('simpleSetup.error.prerequisites.python.title')
            errorMessage.value = t('simpleSetup.error.prerequisites.python.message')
            errorDetails.value = t('simpleSetup.error.prerequisites.python.details')
            currentState.value = 'error'
            return false
          }
        } else {
          console.log("Python sanity check passed");
        }
        
        // Fetch settings, versions, and validate installation path
        return await prepareForInstallation()
      } catch (error) {
        console.error('Failed to check prerequisites:', error)
        errorTitle.value = t('simpleSetup.error.system.title')
        errorMessage.value = t('simpleSetup.error.system.message')
        errorDetails.value = error.toString()
        currentState.value = 'error'
        return false
      }
    }

    const startInstallation = async () => {
      currentState.value = 'installing'
      installationProgress.value = 0
      currentStep.value = 0
      installMessages.value = []
      timeStarted.value = new Date()


      try { // tracking should never fail installation
        await invoke("track_event_command", { name: "GUI simple installation started" });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }

      try {
        // Set up event listeners
        unlistenProgress = await listen('installation-progress', async (event) => {
          const { stage, percentage, message, detail, version } = event.payload;

          installationProgress.value = percentage;
          installationMessage.value = message;

          // Update progress with smooth transitions
          if (percentage !== undefined) {
            // Ensure progress only moves forward (avoid jumping backwards)
            if (percentage >= installationProgress.value) {
              installationProgress.value = percentage
            }
          }

          if (message) {
            installationMessage.value = message
          }

          // Map stage to step for UI
          const stageToStep = {
            'checking': 0,
            'prerequisites': 1,
            'download': 2,
            'extract': 3,
            'tools': 4,
            'python': 5,
            'configure': 6,
            'complete': 7
          };

          // For download stage, show submodules step when progress > 10%
          if (stage === 'download' && percentage > 10) {
            currentStep.value = 3 // Show submodules step
            if (stageToStep[stage] !== undefined) {
              updateInstallationStep(stage)
            }
          } else if (stageToStep[stage] !== undefined) {
            // Only update step if we're moving forward
            if (stageToStep[stage] >= currentStep.value) {
              currentStep.value = stageToStep[stage]
              updateInstallationStep(stage) // Update title based on stage
            }
          }

          if (stage === 'error') {
            currentState.value = 'error'
            errorMessage.value = message || 'Installation failed'
            errorDetails.value = detail || ''
            try { // tracking should never fail installation
              await invoke("track_event_command", { name: "GUI simple installation failed", additional_data: { duration_seconds: (new Date() - timeStarted.value) / 1000, version: version, errorMessage: message, errorDetails: detail } });
            } catch (error) {
              console.warn('Failed to track event:', error);
            }
          } else if (stage === 'complete' || percentage === 100) {
            currentState.value = 'complete'
            installationProgress.value = 100 // Ensure we show 100%
            try { // tracking should never fail installation
              await invoke("track_event_command", { name: "GUI simple installation succeeded", additional_data: { duration_seconds: (new Date() - timeStarted.value) / 1000, version: version } });
            } catch (error) {
              console.warn('Failed to track event:', error);
            }
          }

          // Auto-advance to complete state when we reach 100%
          if (percentage >= 100 && stage !== 'error') {
            setTimeout(() => {
              if (currentState.value === 'installing') {
                currentState.value = 'complete'
              }
            }, 1000) // Give a moment to show 100% before completing
          }
        });

        unlistenLog = await listen('log-message', (event) => {
          const { level, message } = event.payload;
          installMessages.value.push(`[${level}] ${message}`);
        });

        // Start installation

        await invoke('start_simple_setup', {
          version: selectedVersion.value,
          path: installPath.value
        })
      } catch (error) {
        currentState.value = 'error'
        errorTitle.value = t('simpleSetup.error.start.title')
        errorMessage.value = error.toString()
      }
    }

    const updateInstallationStep = (step) => {
      // Update installation title based on step
      const titles = {
        'checking': t('simpleSetup.installation.steps.check.title'),
        'prerequisites': t('simpleSetup.installation.steps.prerequisites.title'),
        'download': t('simpleSetup.installation.steps.download.title'),
        'extract': t('simpleSetup.installation.steps.extract.title'),
        'tools': t('simpleSetup.installation.steps.tools.title'),
        'python': t('simpleSetup.installation.steps.python.title'),
        'configure': t('simpleSetup.installation.steps.configure.title'),
        'complete': t('simpleSetup.installation.steps.complete.title')
      }

      if (titles[step]) {
        installationTitle.value = titles[step]
      }
    }

    const retry = async () => {
      currentState.value = 'checking'
      errorMessage.value = ''
      errorDetails.value = ''
      installationProgress.value = 0
      currentStep.value = 0
      installMessages.value = []
      nextTick(() => {
        setTimeout(() => {
          checkPrerequisites(true);
        }, 300);
      });
    }

    const skipAndContinue = async () => {
      // Skip prerequisites verification and proceed with setup
      verificationFailed.value = false
      errorMessage.value = ''
      errorDetails.value = ''
      currentState.value = 'checking'
      
      // Fetch settings, versions, and validate installation path
      await prepareForInstallation()
    }

    const viewLogs = async () => {
      try {
        // await invoke('open_logs_folder')
        let logPath = await invoke("get_logs_folder", {});
        invoke("show_in_folder", { path: logPath });
      } catch (error) {
        message.error(t('simpleSetup.messages.errors.logs'))
      }
    }

    const viewDocumentation = async () => {
      try {
        await openUrl('https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/')
      } catch (error) {
        message.error('Failed to open documentation')
      }
    }

    const openIDE = async () => {
      try {
        await invoke('open_vscode')
      } catch (error) {
        message.info('Please install VS Code with ESP-IDF extension')
      }
    }

    const useWizard = () => {
      router.push('/wizard/1')
    }

    const goToManagement = () => {
      router.push('/version-management')
    }

    const goBack = () => {
      router.push('/basic-installer')
    }

    onMounted(() => {
      nextTick(() => {
        setTimeout(() => {
          checkPrerequisites();
        }, 300);
      });
    })

    onUnmounted(() => {
      if (unlistenProgress) unlistenProgress()
      if (unlistenComplete) unlistenComplete()
      if (unlistenError) unlistenError()
      if (unlistenLog) unlistenLog()
    })

    return {
      currentState,
      currentStep,
      selectedVersion,
      installPath,
      installationTitle,
      installationMessage,
      installationProgress,
      installMessages,
      installationSteps,
      errorTitle,
      errorMessage,
      errorDetails,
      verificationFailed,
      getStatusIcon,
      getStatusIconClass,
      getProgressColorScheme,
      startInstallation,
      retry,
      skipAndContinue,
      viewLogs,
      viewDocumentation,
      openIDE,
      useWizard,
      goToManagement,
      goBack,
      appStore
    }
  }
}
</script>

<style scoped>
.simple-setup {
  padding: 2rem;
  /* max-width: 900px; */
  margin: 0 auto;
}

.setup-header {
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

.status-card {
  background: white;
  padding: 2rem;
}

/* Checking Status */
.checking-status {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 3rem 0;
  gap: 1rem;
}

.checking-status h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.5rem;
  color: #1f2937;
  margin: 0;
}

.checking-status p {
  color: #6b7280;
  font-size: 1rem;
}

/* Ready Status */
.ready-status {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 1.5rem;
}

.ready-status h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.75rem;
  color: #1f2937;
  margin: 0;
}

.installation-summary {
  background: #f9fafb;
  border-radius: 8px;
  padding: 1.5rem;
  width: 100%;
  max-width: 500px;
}

.summary-item {
  display: flex;
  justify-content: space-between;
  padding: 0.5rem 0;
  border-bottom: 1px solid #e5e7eb;
}

.summary-item:last-child {
  border-bottom: none;
}

.summary-label {
  font-weight: 500;
  color: #6b7280;
}

.summary-value {
  color: #1f2937;
  font-weight: 600;
}

/* Installing Status */
.installing-status {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.status-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  justify-content: center;
}

.status-header h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.5rem;
  color: #1f2937;
  margin: 0;
}

.status-description {
  text-align: center;
  color: #6b7280;
  margin: 0;
}

.rotating {
  animation: rotate 2s linear infinite;
}

@keyframes rotate {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

/* Complete/Error Status */
.complete-status,
.error-status {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.completion-actions,
.error-actions {
  display: flex;
  gap: 1rem;
  justify-content: center;
  flex-wrap: wrap;
}

.post-install-info {
  background: #f9fafb;
  border-radius: 8px;
  padding: 1.5rem;
  text-align: left;
}

.post-install-info h3 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.125rem;
  color: #1f2937;
  margin: 0 0 1rem 0;
}

.post-install-info ol {
  padding-left: 1.5rem;
  color: #4b5563;
  line-height: 1.8;
}

.post-install-info code {
  background: #e5e7eb;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-family: monospace;
  font-size: 0.875rem;
}

.log-content,
.error-details {
  font-family: monospace;
  font-size: 0.875rem;
  line-height: 1.5;
  color: #374151;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.error-details {
  color: #991b1b;
}

.n-button[type="primary"] {
  background-color: #E8362D;
  color: #e5e7eb;
}

.setup-header .n-button {
  color: white !important;
}
</style>
