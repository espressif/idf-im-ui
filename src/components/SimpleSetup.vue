<template>
  <div class="simple-setup">
    <div class="setup-header">
      <h1 class="title">{{ $t('simpleSetup.title') }}</h1>
      <n-button @click="goBack" quaternary v-if="currentState !== 'installing'" text-color="white">
        <template #icon>
          <n-icon>
            <ArrowLeftOutlined />
          </n-icon>
        </template>
        {{ $t('simpleSetup.back') }}
      </n-button>
    </div>

    <!-- Pre-installation Check (prerequisites + python) -->
    <n-card v-if="currentState === 'checking'" class="status-card">
      <div class="checking-status">
        <n-spin size="large" />
        <h2>{{ $t('simpleSetup.preparation.title') }}</h2>
        <p>{{ $t('simpleSetup.preparation.checking') }}</p>
      </div>
    </n-card>

    <!-- Loading available packages -->
    <n-card v-else-if="currentState === 'loading'" class="status-card">
      <div class="checking-status">
        <n-spin size="large" />
        <h2>{{ $t('simpleSetup.preparation.title') }}</h2>
        <p>{{ $t('simpleSetup.loading.packages') }}</p>
      </div>
    </n-card>

    <!-- Choose package (+ drive on Windows) -->
    <n-card v-else-if="currentState === 'select'" class="status-card">
      <div class="ready-status">
        <n-icon :size="64" color="#52c41a">
          <CheckCircleOutlined />
        </n-icon>
        <h2>{{ $t('simpleSetup.ready.title') }}</h2>

        <div class="select-block">
          <label class="field-label">{{ $t('simpleSetup.select.version') }}</label>
          <n-select v-model:value="selectedVersion" :options="versionOptions" />
        </div>

        <!-- Windows-only: change the install DRIVE -->
        <div class="select-block" v-if="appStore.os === 'windows' && drives.length > 1">
          <n-checkbox v-model:checked="allowDriveChange">
            {{ $t('simpleSetup.drive.acknowledge') }}
          </n-checkbox>
          <div v-if="allowDriveChange" class="drive-picker">
            <label class="field-label">{{ $t('simpleSetup.drive.label') }}</label>
            <n-select v-model:value="selectedDrive" :options="driveOptions" />
            <n-alert type="warning" :bordered="false" style="margin-top: 0.5rem;">
              {{ $t('simpleSetup.drive.warning', { drive: selectedDrive }) }}
            </n-alert>
          </div>
        </div>

        <div class="installation-summary">
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.drive.installLocation') }}:</span>
            <span class="summary-value mono">{{ displayPath }}</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.ready.summary.size') }}:</span>
            <span class="summary-value">{{ selectedArchive ? formatSize(selectedArchive.size) : '—' }}</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">{{ $t('simpleSetup.ready.summary.time') }}:</span>
            <span class="summary-value">{{ $t('simpleSetup.select.estimatedTime') }}</span>
          </div>
        </div>

        <n-alert type="info" :bordered="false" style="margin: 1.5rem 0;">
          {{ $t('simpleSetup.select.offlineAlert') }}
        </n-alert>

        <n-button @click="startInstallation" type="primary" size="large" block
          :disabled="!selectedVersion" data-id="start-simple-offline-button">
          {{ $t('simpleSetup.ready.startButton') }}
        </n-button>
      </div>
    </n-card>

    <!-- Installation Progress (two phases: download, then install with detailed steps) -->
    <n-card v-else-if="currentState === 'installing'" class="status-card">
      <div class="installing-status">
        <div class="status-header">
          <n-icon :size="48" :class="getStatusIconClass">
            <component :is="getStatusIcon" />
          </n-icon>
          <h2>{{ phaseTitle }}</h2>
        </div>

        <!-- Two coarse phases -->
        <div class="phase-track">
          <div class="phase-chip" :class="{ active: phase === 'download', done: phase === 'install' }">
            <span class="phase-dot">1</span>{{ $t('simpleSetup.installation.steps.download.title') }}
          </div>
          <div class="phase-sep"></div>
          <div class="phase-chip" :class="{ active: phase === 'install' }">
            <span class="phase-dot">2</span>{{ $t('simpleSetup.installation.steps.install.title') }}
          </div>
        </div>

        <p class="status-description">{{ installationMessage }}</p>

        <!-- Show simple progress bar during DOWNLOAD -->
        <n-progress v-if="!installPhaseStarted" type="line" :percentage="downloadProgress"
          :processing="downloadProgress < 100" :indicator-placement="'inside'" color="#E8362D" />

        <!-- Show detailed GlobalProgress during INSTALL -->
        <GlobalProgress
          v-else
          :initial-message="installationMessage"
          :initial-progress="installProgress"
          :show-details="true"
          :color-scheme="getProgressColorScheme"
          :steps="installationSteps"
          event-channel="installation-progress"
        />

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

        <!-- Keep / delete the downloaded archive -->
        <div class="archive-prompt" v-if="downloadedArchivePath && archiveDecision === ''">
          <h3>{{ $t('simpleSetup.archive.title') }}</h3>
          <p>{{ $t('simpleSetup.archive.question') }}</p>
          <p class="archive-path">{{ downloadedArchivePath }}</p>
          <div class="archive-actions">
            <n-button @click="keepArchive" :disabled="isDeletingArchive" size="large">
              {{ $t('simpleSetup.archive.keep') }}
            </n-button>
            <n-button @click="deleteArchive" :loading="isDeletingArchive" type="warning" size="large">
              {{ $t('simpleSetup.archive.delete') }}
            </n-button>
          </div>
        </div>
        <n-alert v-else-if="archiveDecision === 'kept'" type="info" :bordered="false" style="margin-top: 1rem;">
          {{ $t('simpleSetup.archive.kept') }} {{ downloadedArchivePath }}
        </n-alert>
        <n-alert v-else-if="archiveDecision === 'deleted'" type="success" :bordered="false" style="margin-top: 1rem;">
          {{ $t('simpleSetup.archive.deleted') }}
        </n-alert>

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

    <!-- Error State (including prereq/python issues) -->
    <n-card v-else-if="currentState === 'error'" class="status-card">
      <div class="error-status">
        <n-result :status="warningLike ? 'warning' : 'error'" :title="errorTitle" :description="errorMessage">
          <template #icon>
            <n-icon :size="72" :color="warningLike ? '#faad14' : '#ff4d4f'">
              <CloseCircleOutlined />
            </n-icon>
          </template>
          <template #footer>
            <CheckResultsList v-if="pythonCheckResults.length > 0" :items="pythonCheckResults" />
            <div class="error-actions">
              <n-button v-if="verificationFailed" @click="skipAndContinue" type="warning" size="large" data-id="skip-prerequisites-button">
                {{ $t('common.prerequisites.skipCheck') }}
              </n-button>
              <n-button v-if="!noArchive" @click="viewLogs" type="info" size="large">
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

        <n-alert v-if="errorDetails" :type="warningLike ? 'warning' : 'error'" style="margin-top: 2rem;">
          <template #header>{{ $t('simpleSetup.error.details') }}</template>
          <pre class="error-details">{{ errorDetails }}</pre>
        </n-alert>
      </div>
    </n-card>
  </div>
</template>

<script>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  NButton, NCard, NIcon, NSpin, NResult, NAlert, NProgress,
  NCollapse, NCollapseItem, NScrollbar, NSelect, NCheckbox, useMessage
} from 'naive-ui'
import {
  ArrowLeftOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  DownloadOutlined,
  ToolOutlined,
  LoadingOutlined
} from '@vicons/antd'
import GlobalProgress from './GlobalProgress.vue'
import CheckResultsList from './CheckResultsList.vue'
import { useAppStore } from '../store'

export default {
  name: 'SimpleSetup',
  components: {
    NButton, NCard, NIcon, NSpin, NResult, NAlert, NProgress,
    NCollapse, NCollapseItem, NScrollbar, NSelect, NCheckbox,
    ArrowLeftOutlined, CheckCircleOutlined, CloseCircleOutlined,
    DownloadOutlined, ToolOutlined, LoadingOutlined,
    GlobalProgress,
    CheckResultsList
  },
  setup() {
    const router = useRouter()
    const message = useMessage()
    const appStore = useAppStore()
    const { t } = useI18n()

    // States: checking -> loading -> select -> installing -> complete | error
    const currentState = ref('checking')

    // Package selection
    const archives = ref([])
    const selectedVersion = ref('')

    // Default install location
    const installPath = ref('')

    // Windows drive override
    const drives = ref([])
    const allowDriveChange = ref(false)
    const selectedDrive = ref('')
    const defaultDrive = ref('')

    // Post-install archive handling
    const downloadedArchivePath = ref('')
    const archiveDecision = ref('')
    const isDeletingArchive = ref(false)

    // Two-phase progress
    const installPhaseStarted = ref(false)
    const downloadProgress = ref(0)
    const installProgress = ref(0)
    const installationMessage = ref('')
    const installMessages = ref([])
    const timeStarted = ref(null)

    // Detailed install steps (for GlobalProgress)
    const installationSteps = ref([
      { title: t('simpleSetup.installation.steps.check.title'), description: t('simpleSetup.installation.steps.check.description') },
      { title: t('simpleSetup.installation.steps.prerequisites.title'), description: t('simpleSetup.installation.steps.prerequisites.description') },
      { title: t('simpleSetup.installation.steps.download.title'), description: t('simpleSetup.installation.steps.download.description') },
      { title: t('simpleSetup.installation.steps.extract.title'), description: t('simpleSetup.installation.steps.extract.description') },
      { title: t('simpleSetup.installation.steps.tools.title'), description: t('simpleSetup.installation.steps.tools.description') },
      { title: t('simpleSetup.installation.steps.python.title'), description: t('simpleSetup.installation.steps.python.description') },
      { title: t('simpleSetup.installation.steps.configure.title'), description: t('simpleSetup.installation.steps.configure.description') },
      { title: t('simpleSetup.installation.steps.complete.title'), description: t('simpleSetup.installation.steps.complete.description') }
    ])

    // Error handling
    const errorTitle = ref('')
    const errorMessage = ref('')
    const errorDetails = ref('')
    const noArchive = ref(false)
    const warningLike = ref(false)
    const pythonCheckResults = ref([])
    const verificationFailed = ref(false)

    let unlistenProgress = null
    let unlistenLog = null

    // Computed
    const versionOptions = computed(() =>
      archives.value.map(a => ({ label: `${a.version}  (${formatSize(a.size)})`, value: a.version }))
    )
    const selectedArchive = computed(() =>
      archives.value.find(a => a.version === selectedVersion.value) || null
    )
    const driveOptions = computed(() => drives.value.map(d => ({ label: d, value: d })))
    const driveChanged = computed(() =>
      appStore.os === 'windows' &&
      allowDriveChange.value &&
      !!selectedDrive.value &&
      selectedDrive.value !== defaultDrive.value
    )
    const displayPath = computed(() =>
      driveChanged.value ? swapDrive(installPath.value, selectedDrive.value) : installPath.value
    )

    const phase = computed(() => (installPhaseStarted.value ? 'install' : 'download'))
    const phaseProgress = computed(() => (installPhaseStarted.value ? installProgress.value : downloadProgress.value))
    const phaseTitle = computed(() =>
      installPhaseStarted.value
        ? (t('simpleSetup.installation.title') || 'Installing ESP-IDF')
        : t('simpleSetup.installation.downloadingTitle')
    )

    const getStatusIcon = computed(() => {
      if (!installPhaseStarted.value) return DownloadOutlined
      if (installProgress.value < 100) return ToolOutlined
      return CheckCircleOutlined
    })
    const getStatusIconClass = computed(() => (phaseProgress.value < 100 ? 'rotating' : ''))
    const getProgressColorScheme = computed(() => {
      if (currentState.value === 'error') return 'error'
      if (installProgress.value === 100) return 'success'
      return 'primary'
    })

    // Utils
    const swapDrive = (p, drive) => {
      if (!p || !drive) return p
      return p.replace(/^[A-Za-z]:/, drive.replace(/:?$/, ':'))
    }

    const formatSize = (bytes) => {
      if (!bytes) return '—'
      const gb = bytes / 1_073_741_824
      if (gb >= 1) return `${gb.toFixed(2)} GB`
      return `${Math.round(bytes / 1_048_576)} MB`
    }

    const trackEvent = async (event, fields = {}) => {
      try { await invoke('track_event_command', { event, mode: 'simple', ...fields }) }
      catch (e) { console.warn('Failed to track event:', e) }
    }

    // === Prerequisites & Python Checks ===
    // Windows: prerequisites (git, python) are auto-installed during the offline
    // install, so we skip the up-front check. Non-Windows: prerequisites must
    // already be present; if missing, the user is steered to the wizard.
    const checkPrerequisites = async (force = false) => {
      try {
        currentState.value = 'checking'
        verificationFailed.value = false
        pythonCheckResults.value = []

        if (appStore.os !== 'windows') {
          if (appStore.prerequisitesLastChecked === null || force) {
            await appStore.checkPrerequisites(force)
          }

          const prereqResult = appStore.prerequisitesStatus

          if (!prereqResult.canVerify) {
            handleVerificationError(prereqResult.shellFailed)
            return false
          }

          if (!prereqResult.allOk) {
            handlePrerequisitesMissing(prereqResult.missing)
            return false
          }

          // Python sanity check
          const results = await invoke('python_sanity_check', {})
          const pythonSane = Array.isArray(results) && results.length > 0 && results.every(r => r.passed)

          if (!pythonSane) {
            pythonCheckResults.value = Array.isArray(results) ? results : []
            errorTitle.value = t('simpleSetup.error.prerequisites.python.title')
            errorMessage.value = t('simpleSetup.error.prerequisites.python.message')
            errorDetails.value = ''
            currentState.value = 'error'
            return false
          }
        }

        // If everything passes (or Windows, where we skipped the check), load archives.
        await loadArchives()
        return true
      } catch (error) {
        console.error('Prerequisite check failed:', error)
        errorTitle.value = t('simpleSetup.error.system.title')
        errorMessage.value = t('simpleSetup.error.system.message')
        errorDetails.value = error.toString()
        currentState.value = 'error'
        return false
      }
    }

    const handleVerificationError = (shellFailed) => {
      verificationFailed.value = true
      warningLike.value = true
      errorTitle.value = t('common.prerequisites.verificationFailedTitle')
      errorMessage.value = shellFailed
        ? t('common.prerequisites.shellFailed')
        : t('common.prerequisites.verificationError')
      errorDetails.value = t('common.prerequisites.verificationFailedHint')
      currentState.value = 'error'
    }

    const handlePrerequisitesMissing = (missing) => {
      warningLike.value = true
      errorTitle.value = t('simpleSetup.error.prerequisites.title')
      errorMessage.value = t('simpleSetup.error.prerequisites.message')

      if (appStore.os === 'windows') {
        errorDetails.value = t('simpleSetup.error.prerequisites.windowsHint') + '\n\n' +
          t('common.prerequisites.windowsHint', { list: missing.join(', ') })
      } else {
        errorDetails.value = t('common.prerequisites.manualHint') + '\n' +
          (appStore.os === 'macos'
            ? t('common.prerequisites.macosHint', { list: missing.join(' ') })
            : t('common.prerequisites.linuxHint', { list: missing.join(' ') }))
      }
      currentState.value = 'error'
    }

    // === Load Archives ===
    const loadArchives = async () => {
      currentState.value = 'loading'
      noArchive.value = false
      try {
        const list = await invoke('get_offline_archives')
        archives.value = Array.isArray(list) ? list : []

        if (archives.value.length === 0) {
          noArchive.value = true
          warningLike.value = true
          errorTitle.value = t('simpleSetup.error.noArchive.title')
          errorMessage.value = t('simpleSetup.error.noArchive.message')
          errorDetails.value = t('simpleSetup.error.noArchive.detail', { platform: appStore.os })
          currentState.value = 'error'
          return
        }

        selectedVersion.value = archives.value[0].version

        const settings = await invoke('get_settings')
        installPath.value = settings?.path || ''

        if (appStore.os === 'windows') {
          const m = installPath.value.match(/^([A-Za-z]):/)
          defaultDrive.value = m ? `${m[1]}:` : ''
          selectedDrive.value = defaultDrive.value
          try {
            drives.value = await invoke('get_available_drives')
          } catch (e) {
            drives.value = []
          }
        }

        currentState.value = 'select'
      } catch (error) {
        console.error('Failed to load offline archives:', error)
        errorTitle.value = t('simpleSetup.error.system.title')
        errorMessage.value = t('simpleSetup.error.system.message')
        errorDetails.value = error.toString()
        currentState.value = 'error'
      }
    }

    // === Event Listeners ===
    const attachListeners = async () => {
      unlistenProgress = await listen('installation-progress', async (event) => {
        const { stage, percentage, message: msg, detail, version } = event.payload
        if (msg) installationMessage.value = msg

        if (stage === 'error') {
          currentState.value = 'error'
          warningLike.value = false
          errorTitle.value = t('simpleSetup.error.start.title')
          errorMessage.value = msg || t('simpleSetup.error.system.message')
          errorDetails.value = detail || ''
          trackEvent('install_finished', {
            outcome: 'failure',
            versions: [version],
            error_message: detail || msg
          })
          return
        }

        if (stage === 'complete') {
          installPhaseStarted.value = true
          installProgress.value = 100
          if (currentState.value === 'installing') currentState.value = 'complete'
          trackEvent('install_finished', {
            outcome: 'success',
            versions: [version]
          })
          return
        }

        if (installPhaseStarted.value) {
          if (percentage != null) installProgress.value = percentage
        } else if (stage === 'download') {
          if (percentage != null) downloadProgress.value = percentage
        } else if (stage) {
          installPhaseStarted.value = true
          if (percentage != null) installProgress.value = percentage
        }
      })

      unlistenLog = await listen('log-message', (event) => {
        const { level, message: msg } = event.payload
        installMessages.value.push(`[${level}] ${msg}`)
      })
    }

    // === Actions ===
    const startInstallation = async () => {
      try {
        const ok = await invoke('is_path_empty_or_nonexistent_command', {
          path: displayPath.value,
          versions: [selectedVersion.value]
        })
        if (!ok) {
          warningLike.value = true
          noArchive.value = false
          errorTitle.value = t('simpleSetup.error.pathNotEmpty.title')
          errorMessage.value = t('simpleSetup.error.pathNotEmpty.message', { path: `${displayPath.value}/${selectedVersion.value}` })
          errorDetails.value = t('simpleSetup.error.pathNotEmpty.detail')
          currentState.value = 'error'
          return
        }
      } catch (e) {
        console.warn('Path check failed:', e)
      }

      currentState.value = 'installing'
      installPhaseStarted.value = false
      downloadProgress.value = 0
      installProgress.value = 0
      installationMessage.value = ''
      installMessages.value = []
      downloadedArchivePath.value = ''
      archiveDecision.value = ''
      timeStarted.value = new Date()

      await trackEvent('install_started', { versions: [selectedVersion.value] })

      try {
        await attachListeners()
        const path = await invoke('start_simple_offline_setup', {
          version: selectedVersion.value,
          drive: driveChanged.value ? selectedDrive.value : null
        })
        downloadedArchivePath.value = path
      } catch (error) {
        if (currentState.value !== 'error') {
          currentState.value = 'error'
          warningLike.value = false
          errorTitle.value = t('simpleSetup.error.start.title')
          errorMessage.value = error.toString()
        }
      }
    }

    const skipAndContinue = async () => {
      verificationFailed.value = false
      errorMessage.value = ''
      errorDetails.value = ''
      await loadArchives()
    }

    const keepArchive = () => { archiveDecision.value = 'kept' }
    const deleteArchive = async () => {
      if (!downloadedArchivePath.value) return
      isDeletingArchive.value = true
      try {
        await invoke('delete_offline_archive', { path: downloadedArchivePath.value })
        archiveDecision.value = 'deleted'
      } catch (error) {
        message.error(t('simpleSetup.archive.deleteFailed'))
      } finally {
        isDeletingArchive.value = false
      }
    }

    const retry = async () => {
      errorMessage.value = ''
      errorDetails.value = ''
      installPhaseStarted.value = false
      downloadProgress.value = 0
      installProgress.value = 0
      installMessages.value = []
      pythonCheckResults.value = []
      await checkPrerequisites(true)
    }

    const viewLogs = async () => {
      try {
        const logPath = await invoke('get_logs_folder', {})
        invoke('show_in_folder', { path: logPath })
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

    const useWizard = () => router.push('/wizard/1')
    const goToManagement = () => router.push('/version-management')
    const goBack = () => router.push('/basic-installer')

    onMounted(() => { checkPrerequisites() })
    onUnmounted(() => {
      if (unlistenProgress) unlistenProgress()
      if (unlistenLog) unlistenLog()
    })

    return {
      currentState,
      archives, selectedVersion, versionOptions, selectedArchive,
      installPath, displayPath,
      drives, driveOptions, allowDriveChange, selectedDrive, defaultDrive,
      downloadedArchivePath, archiveDecision, isDeletingArchive,
      installPhaseStarted, downloadProgress, installProgress,
      installationMessage, installMessages, installationSteps,
      errorTitle, errorMessage, errorDetails, noArchive, warningLike,
      pythonCheckResults, verificationFailed,
      phase, phaseProgress, phaseTitle,
      getStatusIcon, getStatusIconClass, getProgressColorScheme,
      formatSize,
      startInstallation, skipAndContinue,
      keepArchive, deleteArchive,
      retry, viewLogs, viewDocumentation, useWizard, goToManagement, goBack,
      appStore
    }
  }
}
</script>

<style scoped>
.simple-setup {
  padding: 2rem;
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

.select-block {
  width: 100%;
  max-width: 560px;
  text-align: left;
}

.drive-picker {
  margin-top: 0.75rem;
}

.field-label {
  display: block;
  font-weight: 500;
  color: #6b7280;
  margin-bottom: 0.5rem;
}

.installation-summary {
  background: #f9fafb;
  border-radius: 8px;
  padding: 1.5rem;
  width: 100%;
  max-width: 560px;
}

.summary-item {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.5rem 0;
  border-bottom: 1px solid #e5e7eb;
}

.summary-item:last-child {
  border-bottom: none;
}

.summary-label {
  font-weight: 500;
  color: #6b7280;
  white-space: nowrap;
}

.summary-value {
  color: #1f2937;
  font-weight: 600;
  text-align: right;
  word-break: break-all;
}

.mono {
  font-family: monospace;
  font-weight: 500;
}

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

/* Two-phase track */
.phase-track {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
}

.phase-chip {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.9rem;
  border-radius: 999px;
  border: 1px solid #e5e7eb;
  color: #6b7280;
  font-size: 0.9rem;
  font-weight: 500;
}

.phase-chip.active {
  border-color: #E8362D;
  color: #E8362D;
  background: #fdeceb;
}

.phase-chip.done {
  border-color: #10b981;
  color: #10b981;
  background: #f0fdf4;
}

.phase-dot {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: currentColor;
  color: white;
  font-size: 0.7rem;
  font-weight: bold;
}

.phase-sep {
  width: 2rem;
  height: 2px;
  background: #e5e7eb;
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
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

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

.archive-prompt {
  background: #f9fafb;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 1.5rem;
  text-align: center;
}

.archive-prompt h3 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.125rem;
  color: #1f2937;
  margin: 0 0 0.5rem 0;
}

.archive-prompt p {
  color: #4b5563;
  margin: 0 0 0.5rem 0;
}

.archive-path {
  font-family: monospace;
  font-size: 0.8rem;
  color: #6b7280;
  word-break: break-all;
}

.archive-actions {
  display: flex;
  gap: 1rem;
  justify-content: center;
  margin-top: 1rem;
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