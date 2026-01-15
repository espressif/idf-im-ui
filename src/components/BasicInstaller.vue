<template>
  <div class="basic-installer" data-id="basic-installer-container">
    <div class="installer-header">
      <h1 class="title" data-id="basic-installer-title">{{ $t('basicInstaller.title') }}</h1>
      <n-button @click="goBack" quaternary :disabled="isLoading" data-id="back-button">
        <template #icon>
          <n-icon><ArrowLeftOutlined /></n-icon>
        </template>
        {{ $t('basicInstaller.back') }}
      </n-button>
    </div>

    <!-- Loading State -->
    <div v-if="isLoading" class="loading-container" ref="loadingDiv" data-id="loading-container">
      <n-card class="loading-card">
        <div class="loading-content">
          <n-spin size="large" />
          <h2>{{ $t('basicInstaller.loading.title') }}</h2>
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

    <!-- Verification Failed Alert -->
    <n-alert
      v-if="!isLoading && !canVerify && os !== 'unknown'"
      type="warning"
      class="prerequisites-alert"
      data-id="verification-failed-alert"
    >
      <template #header>{{ $t('common.prerequisites.verificationFailedTitle') }}</template>
      <p>{{ shellFailed ? $t('common.prerequisites.shellFailed') : $t('common.prerequisites.verificationError') }}</p>
      <p style="margin-top: 10px;">{{ $t('common.prerequisites.verificationFailedHint') }}</p>
    </n-alert>

    <!-- Prerequisites Alert (only when verification succeeded but prerequisites missing) -->
    <n-alert
      v-if="!isLoading && canVerify && !prerequisitesOk && os !== 'unknown'"
      :type="os === 'windows' ? 'warning' : 'error'"
      class="prerequisites-alert"
      data-id="prerequisites-alert"
    >
      <template #header>{{ $t('basicInstaller.prerequisites.header') }}</template>
      <div v-if="missingPrerequisites.length > 0">
        <p>{{ $t('basicInstaller.prerequisites.message') }}</p>
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
        data-id="install-prerequisites-button"
      >
        {{ $t('basicInstaller.prerequisites.installButton') }}
      </n-button>
      <p v-else style="margin-top: 10px;">
        {{ $t('basicInstaller.prerequisites.manualInstall') }}
      </p>
    </n-alert>

    <!-- Installation Options -->
    <transition name="fade-in" mode="out-in">
      <div v-if="!isLoading" class="installation-options" data-id="installation-options">
        <!-- Easy Mode -->
        <n-card class="option-card easy-mode-card" hoverable @click="startEasyMode" data-id="easy-mode-card">
          <div class="option-content">
            <div class="option-icon easy">
              <n-icon :size="48"><RocketOutlined /></n-icon>
            </div>
            <h2>{{ $t('basicInstaller.cards.easy.title') }}</h2>
            <p class="option-description">
              {{ $t('basicInstaller.cards.easy.description') }}
            </p>
            <ul class="feature-list">
              <li v-for="(feature, index) in easyFeatures" :key="index">{{ feature }}</li>
            </ul>
            <n-button type="primary" size="large" block data-id="easy-mode-button">
              {{ $t('basicInstaller.cards.easy.button') }}
            </n-button>
          </div>
        </n-card>

        <!-- Custom Installation -->
        <n-card class="option-card custom-mode-card" hoverable @click="startWizard" data-id="custom-mode-card">
          <div class="option-content">
            <div class="option-icon custom">
              <n-icon :size="48"><SettingOutlined /></n-icon>
            </div>
            <h2>{{ $t('basicInstaller.cards.custom.title') }}</h2>
            <p class="option-description">
              {{ $t('basicInstaller.cards.custom.description') }}
            </p>
            <ul class="feature-list">
              <li v-for="(feature, index) in customFeatures" :key="index">{{ feature }}</li>
            </ul>
            <n-button type="primary" size="large" block data-id="custom-mode-button">
              {{ $t('basicInstaller.cards.custom.button') }}
            </n-button>
          </div>
        </n-card>

        <!-- Offline Installation -->
        <n-card class="option-card offline-mode-card" hoverable @click="selectOfflineArchive" data-id="offline-mode-card">
          <div class="option-content">
            <div class="option-icon offline">
              <n-icon :size="48"><CloudDownloadOutlined /></n-icon>
            </div>
            <h2>{{ $t('basicInstaller.cards.offline.title') }}</h2>
            <p class="option-description">
              {{ $t('basicInstaller.cards.offline.description') }}
            </p>
            <n-button type="primary" size="large" block data-id="offline-mode-button">
              {{ $t('basicInstaller.cards.offline.button') }}
            </n-button>
            <!-- Hidden input for CI tests control -->
            <input id="eim_offline_installation_input" type="hidden" ref="offlineInputCITests" />
          </div>
      </n-card>

        <!-- Load Configuration -->
        <n-card class="option-card config-mode-card" hoverable @click="loadConfig" data-id="load-config-card">
          <div class="option-content">
            <div class="option-icon config">
              <n-icon :size="48"><FileTextOutlined /></n-icon>
            </div>
            <h2>{{ $t('basicInstaller.cards.config.title') }}</h2>
            <p class="option-description">
              {{ $t('basicInstaller.cards.config.description') }}
            </p>
            <n-button type="primary" size="large" block data-id="load-config-button">
              {{ $t('basicInstaller.cards.config.button') }}
            </n-button>
            <!-- Hidden input for CI tests control -->
           <input id="eim_load_config_input" type="hidden" ref="configInputCITests" />
          </div>
        </n-card>
      </div>
    </transition>
  </div>
</template>

<script>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
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
import { useAppStore } from '../store'

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
    const { t } = useI18n()

    // Loading state
    const isLoading = ref(false)
    const loadingMessage = ref('Checking prerequisites...')
    const loadingProgress = ref(0)

    const os = ref('unknown')
    const prerequisitesOk = ref(true)
    const missingPrerequisites = ref([])
    const canVerify = ref(true)
    const shellFailed = ref(false)

    // Input elements for external tests
    const offlineInputCITests = ref(null);
    const configInputCITests = ref(null);

    const appStore = useAppStore()

    const checkPrerequisites = async () => {
      try {
        isLoading.value = false
        loadingProgress.value = 20

        loadingMessage.value = t('basicInstaller.loading.detectingOS')

        os.value = await appStore.getOs();

        loadingProgress.value = 40

        loadingMessage.value = t('basicInstaller.loading.checkingPrerequisites')
        let prerequisitesStatus = null;
        if (appStore.prerequisitesLastChecked !== null) {
          prerequisitesStatus = appStore.prerequisitesStatus;
        } else {
          prerequisitesStatus = await appStore.checkPrerequisites();
        }
        prerequisitesOk.value = prerequisitesStatus.allOk
        missingPrerequisites.value = prerequisitesStatus.missing || []
        canVerify.value = prerequisitesStatus.canVerify !== false
        shellFailed.value = prerequisitesStatus.shellFailed || false

        loadingProgress.value = 60

        loadingMessage.value = t('basicInstaller.loading.scanningArchives')
        await new Promise(resolve => setTimeout(resolve, 300))
        loadingProgress.value = 80

        loadingMessage.value = t('basicInstaller.loading.verifyingPaths')
        await new Promise(resolve => setTimeout(resolve, 200))
        loadingProgress.value = 100

        setTimeout(() => {
          isLoading.value = false
        }, 300)

      } catch (error) {
        console.error('Failed to check prerequisites:', error)
        message.error(t('basicInstaller.messages.errors.systemRequirements'))
        isLoading.value = false
      }
    }

    const installPrerequisites = async () => {
      try {
        message.info(t('basicInstaller.messages.startingPrerequisites'))
        await invoke('install_prerequisites')
        setTimeout(() => checkPrerequisites(), 3000)
      } catch (error) {
        message.error(t('basicInstaller.messages.errors.prerequisites'))
      }
    }

    const startEasyMode = async () => {
      // Allow proceeding if: prerequisites OK, OR can't verify (user can skip), OR Windows (can auto-install)
      if (!prerequisitesOk.value && canVerify.value && os.value !== 'windows') {
        message.warning(t('basicInstaller.prerequisites.warning'))
        return
      }
      router.push('/simple-setup')
    }

    const startWizard = () => {
      // Allow proceeding if: prerequisites OK, OR can't verify (user can skip), OR Windows (can auto-install)
      if (!prerequisitesOk.value && canVerify.value && os.value !== 'windows') {
        message.warning(t('basicInstaller.prerequisites.warning'))
        return
      }
      router.push('/wizard/1')
    }

    const loadConfig = async () => {
      try {
        let selected = null
        if((configInputCITests.value?.value ?? '').trim().length > 0){
          selected = configInputCITests.value.value;
          configInputCITests.value = null
        } else {
          selected = await open({
            multiple: false,
            filters: [{
              name: 'Configuration',
              extensions: ['json', 'toml']
            }]
          })
        }

        if (selected) {
          invoke("load_settings", { path: selected }).then(() => {
            message.success(t('basicInstaller.messages.configLoaded'))
            router.push('/installation-progress')
          }).catch((error) => {
            console.error('Failed to load configuration:', error)
            message.error(t('configLoadFailed.messages.configLoadFailed'))
          })
        }
      } catch (error) {
        message.error(t('configLoadFailed.messages.configLoadFailed'))
      }
    }

    const selectOfflineArchive = async () => {
      try {
        let selected = null
        if((offlineInputCITests.value?.value ?? '').trim().length > 0){
          selected = offlineInputCITests.value.value;
          offlineInputCITests.value = null
        } else {
          selected = await open({
            multiple: false,
            filters: [{
              name: 'ESP-IDF Archive',
              extensions: ['zst']
            }]
          })
        }
        if (selected) {
          router.push({
            path: '/offline-installer',
            query: { archives: JSON.stringify([selected]) }
          })
        }
      } catch (error) {
        message.error('Failed to select archive file')
      }
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

    const easyFeatures = [
      t('basicInstaller.cards.easy.feature1'),
      t('basicInstaller.cards.easy.feature2'),
      t('basicInstaller.cards.easy.feature3')
    ]

    const customFeatures = [
      t('basicInstaller.cards.custom.feature1'),
      t('basicInstaller.cards.custom.feature2'),
      t('basicInstaller.cards.custom.feature3')
    ]

    return {
      isLoading,
      loadingMessage,
      loadingProgress,
      os,
      prerequisitesOk,
      missingPrerequisites,
      canVerify,
      shellFailed,
      selectOfflineArchive,
      checkPrerequisites,
      installPrerequisites,
      startEasyMode,
      startWizard,
      loadConfig,
      goBack,
      offlineInputCITests,
      configInputCITests,
      easyFeatures,
      customFeatures
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
  margin-bottom: 0.5rem;
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
