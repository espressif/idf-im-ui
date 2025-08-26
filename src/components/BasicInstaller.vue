<template>
  <div class="basic-installer">
    <div class="installer-header">
      <h1 class="title">Install ESP-IDF</h1>
      <n-button @click="goBack" quaternary :disabled="isLoading">
        <template #icon>
          <n-icon><ArrowLeftOutlined /></n-icon>
        </template>
        Back
      </n-button>
    </div>

    <!-- Loading State -->
    <div v-if="isLoading" class="loading-container" ref="loadingDiv">
      <n-card class="loading-card">
        <div class="loading-content">
          <n-spin size="large" />
          <h2>Checking System Requirements</h2>
          <p>{{ loadingMessage }}</p>
          <n-progress
            type="line"
            :percentage="loadingProgress"
            :show-indicator="false"
            processing
            color="#E8362D"
          />
        </div>
      </n-card>
    </div>

    <!-- Prerequisites Alert -->
    <n-alert
      v-if="!isLoading && !prerequisitesOk && os !== 'unknown'"
      :type="os === 'windows' ? 'warning' : 'error'"
      class="prerequisites-alert"
    >
      <template #header>Prerequisites Missing</template>
      <div v-if="missingPrerequisites.length > 0">
        <p>The following prerequisites are missing:</p>
        <ul>
          <li v-for="prereq in missingPrerequisites" :key="prereq">{{ prereq }}</li>
        </ul>
      </div>
      <n-button
        v-if="os === 'windows'"
        @click="installPrerequisites"
        size="small"
        type="warning"
        style="margin-top: 10px;"
      >
        Install Prerequisites Automatically
      </n-button>
      <p v-else style="margin-top: 10px;">
        Please install the missing prerequisites before continuing.
      </p>
    </n-alert>

    <!-- Installation Options -->
    <transition name="fade-in" mode="out-in">
      <div v-if="!isLoading" class="installation-options">
      <!-- Easy Mode -->
      <n-card
        class="option-card easy-mode-card"
        hoverable
        @click="startEasyMode"
      >
        <div class="option-content">
          <div class="option-icon easy">
            <n-icon :size="48"><RocketOutlined /></n-icon>
          </div>
          <h2>Easy Installation</h2>
          <p class="option-description">
            Quick setup with recommended defaults. Perfect for beginners.
          </p>
          <ul class="feature-list">
            <li>Latest stable ESP-IDF version</li>
            <li>Default installation path</li>
            <li>Automatic configuration</li>
          </ul>
          <n-button type="primary" size="large" block>
            Start Easy Installation
          </n-button>
        </div>
      </n-card>

      <!-- Custom Installation -->
      <n-card
        class="option-card custom-mode-card"
        hoverable
        @click="startWizard"
      >
        <div class="option-content">
          <div class="option-icon custom">
            <n-icon :size="48"><SettingOutlined /></n-icon>
          </div>
          <h2>Custom Installation</h2>
          <p class="option-description">
            Full control over installation settings. For advanced users.
          </p>
          <ul class="feature-list">
            <li>Choose ESP-IDF version</li>
            <li>Custom installation path</li>
            <li>Advanced options</li>
          </ul>
          <n-button type="primary" size="large" block>
            Start Configuration Wizard
          </n-button>
        </div>
      </n-card>

      <!-- Offline Installation -->
      <n-card
        class="option-card offline-mode-card"
        hoverable
        @click="showOfflineModal = true"
      >
        <div class="option-content">
          <div class="option-icon offline">
            <n-icon :size="48"><CloudDownloadOutlined /></n-icon>
          </div>
          <h2>Offline Installation</h2>
          <p class="option-description">
            Install from local archive files (.zst). No internet required.
          </p>
          <div
            class="drop-zone"
            :class="{ 'dragging': isDragging }"
            @drop.prevent="handleDrop"
            @dragover.prevent="isDragging = true"
            @dragleave.prevent="isDragging = false"
          >
            <n-icon :size="32"><InboxOutlined /></n-icon>
            <p>Drop .zst archive here or click to browse</p>
          </div>
        </div>
      </n-card>

      <!-- Load Configuration -->
      <n-card
        class="option-card config-mode-card"
        hoverable
        @click="loadConfig"
      >
        <div class="option-content">
          <div class="option-icon config">
            <n-icon :size="48"><FileTextOutlined /></n-icon>
          </div>
          <h2>Load Configuration</h2>
          <p class="option-description">
            Import existing installation configuration file.
          </p>
          <n-button type="primary" size="large" block>
            Browse Configuration File
          </n-button>
        </div>
      </n-card>
    </div>
    </transition>

    <!-- Offline Installation Modal -->
    <n-modal
      v-model:show="showOfflineModal"
      preset="card"
      title="Offline Installation"
      style="width: 600px"
    >
      <div class="offline-modal-content">
        <n-upload
          v-model:file-list="fileList"
          :accept="'.zst'"
          :max="5"
          @change="handleFileChange"
          directory-dnd
        >
          <n-upload-dragger>
            <div style="margin-bottom: 12px">
              <n-icon size="48" :depth="3">
                <InboxOutlined />
              </n-icon>
            </div>
            <n-text style="font-size: 16px">
              Click or drag .zst archive files here
            </n-text>
            <n-p depth="3" style="margin: 8px 0 0 0">
              Select one or more ESP-IDF offline installation archives
            </n-p>
          </n-upload-dragger>
        </n-upload>

        <div v-if="fileList.length > 0" class="file-list-preview">
          <h4>Selected Archives:</h4>
          <n-tag
            v-for="file in fileList"
            :key="file.id"
            closable
            @close="removeFile(file)"
            style="margin: 4px"
          >
            {{ file.name }}
          </n-tag>
        </div>

        <div class="modal-actions">
          <n-button @click="showOfflineModal = false">Cancel</n-button>
          <n-button
            type="primary"
            @click="proceedOfflineInstall"
            :disabled="fileList.length === 0"
          >
            Proceed with Installation
          </n-button>
        </div>
      </div>
    </n-modal>
  </div>
</template>

<script>
import { ref, onMounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import {
  NButton, NCard, NIcon, NAlert, NModal, NUpload,
  NUploadDragger, NText, NP, NTag, NSpin, NProgress, useMessage
} from 'naive-ui'
import {
  ArrowLeftOutlined,
  RocketOutlined,
  SettingOutlined,
  CloudDownloadOutlined,
  FileTextOutlined,
  InboxOutlined
} from '@vicons/antd'

const loadingDiv = ref(null);

export default {
  name: 'BasicInstaller',
  components: {
    NButton, NCard, NIcon, NAlert, NModal, NUpload,
    NUploadDragger, NText, NP, NTag, NSpin, NProgress,
    ArrowLeftOutlined, RocketOutlined, SettingOutlined,
    CloudDownloadOutlined, FileTextOutlined, InboxOutlined
  },
  setup() {
    const router = useRouter()
    const message = useMessage()

    // Loading state
    const isLoading = ref(false)
    const loadingMessage = ref('Checking prerequisites...')
    const loadingProgress = ref(0)

    const os = ref('unknown')
    const prerequisitesOk = ref(true)
    const missingPrerequisites = ref([])
    const showOfflineModal = ref(false)
    const isDragging = ref(false)
    const fileList = ref([])

    const checkPrerequisites = async () => {
      try {
        isLoading.value = false
        loadingProgress.value = 20

        // Check OS
        loadingMessage.value = 'Detecting operating system...'
        os.value = await invoke('get_operating_system')
        loadingProgress.value = 40

        // Check prerequisites
        loadingMessage.value = 'Checking prerequisites...'
        invoke('check_prerequisites_detailed').then(res => {
          prerequisitesOk.value = res.all_ok
          missingPrerequisites.value = res.missing || []
        })
        loadingProgress.value = 60

        // Check for offline archives
        loadingMessage.value = 'Scanning for offline archives...'
        await new Promise(resolve => setTimeout(resolve, 300)) // Small delay for smoother UX
        loadingProgress.value = 80

        // Check installation paths
        loadingMessage.value = 'Verifying installation paths...'
        await new Promise(resolve => setTimeout(resolve, 200))
        loadingProgress.value = 100

        // Add small delay before hiding loading state for smoother transition
        setTimeout(() => {
          isLoading.value = false
        }, 300)

      } catch (error) {
        console.error('Failed to check prerequisites:', error)
        message.error('Failed to check system requirements')
        isLoading.value = false
      }
    }

    const installPrerequisites = async () => {
      try {
        message.info('Starting prerequisites installation...')
        await invoke('install_prerequisites')
        setTimeout(() => checkPrerequisites(), 3000)
      } catch (error) {
        message.error('Failed to install prerequisites')
      }
    }

    const startEasyMode = async () => {
      if (!prerequisitesOk.value && os.value !== 'windows') {
        message.warning('Please install prerequisites first')
        return
      }
      router.push('/simple-setup')
    }

    const startWizard = () => {
      if (!prerequisitesOk.value && os.value !== 'windows') {
        message.warning('Please install prerequisites first')
        return
      }
      router.push('/wizard/1')
    }

    const loadConfig = async () => {
      try {
        const selected = await open({
          multiple: false,
          filters: [{
            name: 'Configuration',
            extensions: ['json', 'toml']
          }]
        })

        if (selected) {
          const _ = await invoke("load_settings", { path: selected });
          message.success('Configuration loaded')
          router.push('/installation-progress')
        }
      } catch (error) {
        message.error('Failed to load configuration')
      }
    }

    const handleDrop = (e) => {
      isDragging.value = false
      const files = Array.from(e.dataTransfer.files)
      const zstFiles = files.filter(f => f.name.endsWith('.zst'))

      if (zstFiles.length > 0) {
        fileList.value = zstFiles.map(f => ({
          id: Math.random(),
          name: f.name,
          path: f.path,
          file: f
        }))
        showOfflineModal.value = true
      } else {
        message.warning('Please drop .zst archive files only')
      }
    }

    const handleFileChange = (options) => {
      fileList.value = options.fileList
    }

    const removeFile = (file) => {
      fileList.value = fileList.value.filter(f => f.id !== file.id)
    }

    const proceedOfflineInstall = () => {
      const paths = fileList.value.map(f => f.path || f.file?.path)
      router.push({
        path: '/offline-installer',
        query: { archives: JSON.stringify(paths) }
      })
    }

    const goBack = () => {
      router.push('/version-management')
    }

    onMounted(() => {
      // nextTick(() => {
      //   setTimeout(() => {
      //     checkPrerequisites();
      //   }, 300);
      // });
    })

    return {
      isLoading,
      loadingMessage,
      loadingProgress,
      os,
      prerequisitesOk,
      missingPrerequisites,
      showOfflineModal,
      isDragging,
      fileList,
      checkPrerequisites,
      installPrerequisites,
      startEasyMode,
      startWizard,
      loadConfig,
      handleDrop,
      handleFileChange,
      removeFile,
      proceedOfflineInstall,
      goBack
    }
  }
}
</script>

<style scoped>
.basic-installer {
  padding: 2rem;
  max-width: 1200px;
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

.installer-header .n-button {
  color: white;
}

.prerequisites-alert {
  margin-bottom: 2rem;
}

.prerequisites-alert ul {
  margin: 0.5rem 0;
  padding-left: 1.5rem;
}

/* Loading State */
.loading-container {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 400px;
}

.loading-card {
  width: 100%;
  max-width: 500px;
  background: white;
  border: 1px solid #e5e7eb;
}

.loading-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 3rem 2rem;
  gap: 1.5rem;
}

.loading-content h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.5rem;
  color: #1f2937;
  margin: 0;
}

.loading-content p {
  color: #6b7280;
  font-size: 1rem;
  margin: 0;
}

.loading-content .n-progress {
  width: 100%;
  max-width: 300px;
}

/* Installation Options */

.installation-options {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
  gap: 1.5rem;
}

.option-card {
  transition: all 0.3s ease;
  border: 2px solid transparent;
  flex-grow: 1;
}

.option-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
}

.option-card .n-button {
  margin-top: auto;
  margin-bottom: 0rem;
  flex: 0 1 auto;
}

.easy-mode-card:hover {
  border-color: #E8362D;
}

.custom-mode-card:hover {
  border-color: #1290d8;
}

.offline-mode-card:hover {
  border-color: #52c41a;
}

.config-mode-card:hover {
  border-color: #722ed1;
}

.option-content {
  display: flex;
  height: 100%;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 1rem;
}

.option-icon {
  margin-bottom: 1rem;
  padding: 1rem;
  border-radius: 12px;
}

.option-icon.easy {
  background: linear-gradient(135deg, #ff6b6b, #E8362D);
  color: white;
}

.option-icon.custom {
  background: linear-gradient(135deg, #667eea, #1290d8);
  color: white;
}

.option-icon.offline {
  background: linear-gradient(135deg, #48bb78, #38a169);
  color: white;
}

.option-icon.config {
  background: linear-gradient(135deg, #9f7aea, #722ed1);
  color: white;
}

.option-content h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.5rem;
  color: #1f2937;
  margin: 0 0 0.5rem 0;
}

.option-description {
  color: #6b7280;
  margin-bottom: 0rem;
  min-height: 1rem;
}

.feature-list {
  text-align: left;
  margin: 0.5rem 0;
  padding-left: 1.5rem;
  color: #4b5563;
  font-size: 0.9rem;
  flex-grow: 1;
}

.feature-list li {
  margin: 0.5rem 0;
}

.drop-zone {
  border: 2px dashed #d1d5db;
  border-radius: 8px;
  padding: 2rem;
  margin: 1rem 0;
  transition: all 0.3s ease;
  cursor: pointer;
}

.drop-zone.dragging {
  border-color: #52c41a;
  background: #f6ffed;
}

.drop-zone p {
  margin: 0.5rem 0 0 0;
  color: #6b7280;
}

.offline-modal-content {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.file-list-preview h4 {
  margin-bottom: 0.5rem;
  color: #374151;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}

.n-button[type="primary"] {
  background-color: #E8362D;
}

/* Fade transition */
.fade-in-enter-active,
.fade-in-leave-active {
  transition: opacity 0.3s ease;
}

.fade-in-enter-from,
.fade-in-leave-to {
  opacity: 0;
}
</style>
