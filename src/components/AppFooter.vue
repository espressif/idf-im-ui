<template>
  <footer class="app-footer">
    <div class="footer-content">
      <div class="footer-section">
        <span class="version-info">
          ESP-IDF Installation Manager v{{ appVersion }}
        </span>
      </div>

      <div class="footer-section center">
        <n-button
          @click="openDocumentation"
          text
          tag="a"
          size="small"
        >
          <template #icon>
            <n-icon><BookOutlined /></n-icon>
          </template>
          Documentation
        </n-button>

        <n-divider vertical />

        <n-button
          @click="openLogsFolder"
          text
          size="small"
        >
          <template #icon>
            <n-icon><FolderOpenOutlined /></n-icon>
          </template>
          Logs
        </n-button>

        <n-divider vertical />

        <n-button
          @click="reportIssue"
          text
          size="small"
        >
          <template #icon>
            <n-icon><BugOutlined /></n-icon>
          </template>
          Report Issue
        </n-button>

        <n-divider vertical />

        <n-button
          @click="showAbout"
          text
          size="small"
        >
          <template #icon>
            <n-icon><InfoCircleOutlined /></n-icon>
          </template>
          About
        </n-button>
      </div>

      <div class="footer-section">
        <span class="copyright">
          © 2024 Espressif Systems
        </span>
      </div>
    </div>

    <!-- About Modal -->
    <n-modal
      v-model:show="showAboutModal"
      preset="card"
      title="About ESP-IDF Installation Manager"
      style="width: 500px"
    >
      <div class="about-content">
        <div class="about-logo">
          <img src="../assets/espressif_logo.svg" alt="Espressif" />
        </div>

        <div class="about-info">
          <h3>ESP-IDF Installation Manager</h3>
          <p>Version {{ appVersion }}</p>
          <!-- <p class="build-info">Build: {{ buildInfo }}</p> -->
        </div>

        <div class="about-description">
          <p>
            A cross-platform tool for installing and managing ESP-IDF development environment.
          </p>
          <p>
            Supports Windows, macOS, and Linux platforms with both online and offline installation modes.
          </p>
        </div>

        <div class="about-links">
          <n-button @click="openGitHub" type="primary" block>
            <template #icon>
              <n-icon><GithubOutlined /></n-icon>
            </template>
            View on GitHub
          </n-button>
        </div>
      </div>
    </n-modal>

    <!-- Report Issue Modal -->
    <n-modal
      v-model:show="showReportModal"
      preset="card"
      title="Report an Issue"
      style="width: 600px"
    >
      <div class="report-content">
        <n-alert type="info" :bordered="false" style="margin-bottom: 1rem;">
          This will create a log bundle and open the issue reporting page.
        </n-alert>

        <div class="report-info">
          <h4>System Information</h4>
          <div class="system-info">
            <div class="info-row">
              <span class="info-label">OS:</span>
              <span>{{ systemInfo.os }}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Architecture:</span>
              <span>{{ systemInfo.arch }}</span>
            </div>
            <div class="info-row">
              <span class="info-label">App Version:</span>
              <span>{{ appVersion }}</span>
            </div>
          </div>
        </div>

        <n-checkbox v-model:checked="includeLogs" style="margin: 1rem 0;">
          Include installation logs in report
        </n-checkbox>

        <div class="modal-actions">
          <n-button @click="showReportModal = false" class="cancel-button">
            Cancel
          </n-button>
          <n-button
            @click="generateReport"
            type="primary"
            :loading="generatingReport"
          >
            Generate Report & Open Issue
          </n-button>
        </div>
      </div>
    </n-modal>
  </footer>
</template>

<script>
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener';

import {
  NButton, NIcon, NDivider, NModal, NAlert, NCheckbox, useMessage
} from 'naive-ui'
import {
  BookOutlined,
  FolderOpenOutlined,
  BugOutlined,
  InfoCircleOutlined,
  GithubOutlined
} from '@vicons/antd'
import { useAppStore } from '../store'

export default {
  name: 'AppFooter',
  components: {
    NButton, NIcon, NDivider, NModal, NAlert, NCheckbox,
    BookOutlined, FolderOpenOutlined, BugOutlined,
    InfoCircleOutlined, GithubOutlined
  },
  setup() {
    const message = useMessage()

    const appVersion = ref('0.3.0')
    const buildInfo = ref('')
    const showAboutModal = ref(false)
    const showReportModal = ref(false)
    const includeLogs = ref(true)
    const generatingReport = ref(false)
    const appStore = useAppStore()

    const systemInfo = ref({
      os: 'Unknown',
      arch: 'Unknown',
      cpuCount: 'Unknown'
    })

    const getAppInfo = async () => {
      try {
        const info = await invoke('get_app_info')
        appVersion.value = info.version
        // buildInfo.value = info.build_info || `${info.build_date} - ${info.commit_hash?.substring(0, 7)}`
      } catch (error) {
        console.error('Failed to get app info:', error)
      }
    }

    const getSystemInfo = async () => {
      try {
        const os = await invoke('get_operating_system')
        const arch = await invoke('get_system_arch')
        const cpuCount = await invoke('cpu_count')
        systemInfo.value = { os, arch, cpuCount }
        appStore.setSystemInfo(systemInfo.value)
      } catch (error) {
        console.error('Failed to get system info:', error)
      }
    }

    const openDocumentation = async () => {
      try {
        await openUrl('https://docs.espressif.com/projects/idf-im-ui/en/latest/')
      } catch (error) {
        message.error('Failed to open documentation')
        console.log(error)
      }
    }

    const openLogsFolder = async () => {
      try {
        let logPath = await invoke("get_logs_folder", {});
        invoke("show_in_folder", { path: logPath });
        message.success('Logs folder opened')
      } catch (error) {
        message.error('Failed to open logs folder')
      }
    }

    const reportIssue = () => {
      showReportModal.value = true
    }

    const generateReport = async () => {
      generatingReport.value = true

      try {
        // Generate log bundle
        const bundlePath = await invoke('generate_log_bundle', {
          includeLogs: includeLogs.value
        })

        message.success('Log bundle created: ' + bundlePath)

        // Open GitHub issue page with template
        const issueTitle = encodeURIComponent('[Bug Report] Issue with ESP-IDF Installation')
        const issueBody = encodeURIComponent(`
## System Information
- **OS**: ${systemInfo.value.os}
- **Architecture**: ${systemInfo.value.arch}
- **App Version**: ${appVersion.value}

## Description
Please describe the issue you encountered:

## Steps to Reproduce
1.
2.
3.

## Expected Behavior

## Actual Behavior

## Logs
${includeLogs.value ? `Log bundle has been generated at: ${bundlePath}` : 'No logs included'}

## Additional Information
        `.trim())

        await openUrl(`https://github.com/espressif/idf-im-ui/issues/new?title=${issueTitle}&body=${issueBody}`)

        showReportModal.value = false
      } catch (error) {
        message.error('Failed to generate report: ' + error)
      } finally {
        generatingReport.value = false
      }
    }

    const showAbout = () => {
      showAboutModal.value = true
    }

    const openGitHub = async () => {
      try {
        await openUrl('https://github.com/espressif/idf-im-ui')
      } catch (error) {
        message.error('Failed to open GitHub page')
      }
    }

    onMounted(() => {
      getAppInfo()
      getSystemInfo()
    })

    return {
      appVersion,
      buildInfo,
      showAboutModal,
      showReportModal,
      includeLogs,
      generatingReport,
      systemInfo,
      openDocumentation,
      openLogsFolder,
      reportIssue,
      generateReport,
      showAbout,
      openGitHub
    }
  }
}
</script>

<style scoped>
.app-footer {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  background: white;
  border-top: 1px solid #e5e7eb;
  padding: 0.75rem 1.5rem;
  z-index: 100;
}

.footer-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
  max-width: 1400px;
  margin: 0 auto;
}

.footer-section {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.footer-section.center {
  flex: 1;
  justify-content: center;
}

/* Fix footer button styling */
.footer-section .n-button {
  background: transparent !important;
  border: none !important;
  color: #6b7280 !important;
  font-size: 0.875rem;
  padding: 0.25rem 0.5rem;
  transition: color 0.2s ease;
}

.footer-section .n-button:hover {
  background: transparent !important;
  color: #1f2937 !important;
}

.footer-section .n-button .n-icon {
  color: inherit !important;
  margin-right: 0.25rem;
}

.version-info, .copyright {
  font-size: 0.875rem;
  color: #6b7280;
}

.n-divider {
  height: 1rem;
  background: #e5e7eb !important;
}

/* About Modal */
.about-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 1.5rem;
}

.about-logo img {
  width: 80px;
  height: auto;
}

.about-info h3 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.25rem;
  color: #1f2937;
  margin: 0 0 0.5rem 0;
}

.about-info p {
  margin: 0.25rem 0;
  color: #4b5563;
}

.build-info {
  font-size: 0.875rem;
  color: #9ca3af;
  font-family: monospace;
}

.about-description p {
  margin: 0.5rem 0;
  color: #6b7280;
  line-height: 1.5;
}

/* Report Modal */
.report-content {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.system-info {
  background: #f9fafb;
  border-radius: 6px;
  padding: 1rem;
  margin-top: 0.5rem;
}

.info-row {
  display: flex;
  justify-content: space-between;
  padding: 0.25rem 0;
  font-size: 0.875rem;
}

.info-label {
  font-weight: 500;
  color: #6b7280;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}

.cancel-button {
  color: #e5e7eb;
}

/* Modal button styles - only apply to buttons inside modals */
.about-content .n-button[type="primary"],
.modal-actions .n-button[type="primary"] {
  background-color: #E8362D;
}
</style>
