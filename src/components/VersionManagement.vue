<template>
  <div class="version-management" data-id="version-management-container">
    <div class="management-header">
      <h1 class="title" data-id="version-management-title">{{ t('versionManagement.title') }}</h1>
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
      data-id="prerequisites-alert"
    >
      <template #header>{{ t('versionManagement.prerequisites.missing') }}</template>
      {{ t('versionManagement.prerequisites.windowsMessage') }}
      <n-button @click="installPrerequisites" size="small" type="warning" style="margin-left: 10px;" data-id="install-prerequisites-button">
        {{ t('versionManagement.prerequisites.installButton') }}
      </n-button>
    </n-alert>

    <!-- Installed Versions -->
    <div v-if="installedVersions.length > 0" class="versions-section" data-id="installed-versions-section">
      <h2>{{ t('versionManagement.sections.installedVersions') }}</h2>
      <div class="version-cards">
        <n-card
          v-for="version in installedVersions"
          :key="version.id"
          class="version-card"
          hoverable
          :data-id="`version-card-${version.id}`"
        >
          <div class="version-card-content">
            <div class="version-info">
              <h3 :data-id="`version-name-${version.id}`">{{ version.name }}</h3>
              <!-- <n-tag :type="version.active ? 'success' : 'default'" size="small">
                {{ version.version }}
              </n-tag> -->
            </div>
            <div class="version-path" :data-id="`version-path-${version.id}`">
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
                <n-button @click="openIDFTerminal(version)" quaternary circle :data-id="`open-idf-terminal-button-${version.id}`">
                  <template #icon>
                    <n-icon><LaptopOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.openTerminal') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="renameVersion(version)" quaternary circle :data-id="`rename-version-button-${version.id}`">
                  <template #icon>
                    <n-icon><EditOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.rename') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="fixVersion(version)" quaternary circle :data-id="`fix-version-button-${version.id}`">
                  <template #icon>
                    <n-icon><ToolOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.fix') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="openInExplorer(version)" quaternary circle :data-id="`open-in-explorer-button-${version.id}`">
                  <template #icon>
                    <n-icon><FolderOpenOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.openFolder') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="openListTools(version)" quaternary circle :data-id="`list-tools-button-${version.id}`">
                  <template #icon>
                    <n-icon><UnorderedListOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.listTools') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="openListFeatures(version)" quaternary circle :data-id="`list-features-button-${version.id}`">
                  <template #icon>
                    <n-icon><AppstoreOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.listFeatures') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="removeVersion(version)" quaternary circle :data-id="`remove-version-button-${version.id}`" type="error">
                  <template #icon>
                    <n-icon><DeleteOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.remove') }}
            </n-tooltip>
            <n-tooltip v-if="version.installationConfig" trigger="hover">
              <template #trigger>
                <n-button @click="exportInstallationConfig(version)" quaternary circle :data-id="`export-config-button-${version.id}`">
                  <template #icon>
                    <n-icon><SaveOutlined /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ t('versionManagement.version.actions.exportConfig') }}
            </n-tooltip>
          </div>
        </n-card>
      </div>
    </div>

    <!-- No Versions Installed -->
    <div v-else class="empty-state" data-id="no-versions-installed-empty-state">
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
      <n-button @click="goToBasicInstaller" type="primary" size="large" data-id="install-new-version-button">
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
        data-id="install-drivers-button"
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
        data-id="purge-all-button"
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
      data-id="rename-version-modal"
    >
      <n-input
        v-model:value="newVersionName"
        :placeholder="t('versionManagement.modals.rename.placeholder')"
        @keyup.enter="confirmRename"
        data-id="rename-version-input"
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
      data-id="remove-version-modal"
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
      data-id="fix-version-modal"
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
      data-id="purge-all-modal"
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
      <n-checkbox v-model:checked="purgeConfirmed" data-id="purge-all-confirm-checkbox">
        {{ t('versionManagement.modals.purge.confirmation') }}
      </n-checkbox>
    </n-modal>

    <n-modal
      v-model:show="showListToolsModal"
      preset="card"
      :title="t('versionManagement.modals.listTools.title', { name: listToolsVersion?.name })"
      style="max-width: 900px;"
      :bordered="false"
      size="huge"
      data-id="list-tools-modal"
    >
      <div v-if="listToolsLoading" class="list-tools-loading" data-id="list-tools-loading">
        <n-spin :size="32" />
        <p>{{ t('versionManagement.modals.listTools.loading') }}</p>
      </div>

      <div v-else-if="listToolsReport" class="list-tools-content" data-id="list-tools-content">
        <!-- Meta strip + add-tools trigger, aligned on the same row -->
        <div class="list-tools-header">
          <div class="list-tools-meta">
            <div class="meta-row">
              <span class="meta-label">{{ t('versionManagement.modals.listTools.idfPath') }}:</span>
              <code class="meta-value">{{ listToolsReport.idf.path }}</code>
            </div>
            <div class="meta-row">
              <span class="meta-label">{{ t('versionManagement.modals.listTools.toolsPath') }}:</span>
              <code class="meta-value">{{ listToolsReport.idf_tools_path }}</code>
            </div>
          </div>

          <n-button
            v-if="!showAddToolsPanel"
            size="small"
            type="primary"
            secondary
            @click="openAddToolsPanel"
            data-id="show-add-tools-button"
          >
            <template #icon><n-icon><PlusCircleOutlined /></n-icon></template>
            {{ t('versionManagement.modals.listTools.addTools.button') }}
          </n-button>
        </div>

        <!-- Add more tools panel -->
        <div v-if="showAddToolsPanel" class="add-tools-panel" data-id="add-tools-panel">
          <h4>{{ t('versionManagement.modals.listTools.addTools.title') }}</h4>

          <n-empty
            v-if="availableToolsToAdd.length === 0"
            :description="t('versionManagement.modals.listTools.addTools.none')"
            size="small"
            data-id="add-tools-none"
          />

          <n-checkbox-group v-else v-model:value="selectedExtraTools" data-id="add-tools-checkbox-group">
            <n-space vertical>
              <n-checkbox
                v-for="entry in availableToolsToAdd"
                :key="entry.tool.name"
                :value="entry.tool.name"
                :data-id="`add-tools-checkbox-${entry.tool.name}`"
              >
                <strong>{{ entry.tool.name }}</strong> — {{ entry.tool.description }}
              </n-checkbox>
            </n-space>
          </n-checkbox-group>

          <div class="add-tools-actions">
            <n-button size="small" @click="cancelAddToolsPanel" data-id="add-tools-cancel-button">
              {{ t('versionManagement.modals.listTools.addTools.cancel') }}
            </n-button>
            <n-button
              size="small"
              type="primary"
              :disabled="selectedExtraTools.length === 0"
              :loading="addingTools"
              @click="confirmAddTools"
              data-id="add-tools-confirm-button"
            >
              {{ t('versionManagement.modals.listTools.addTools.confirm') }}
            </n-button>
          </div>
        </div>

        <!-- Outdated — now a single compact warning line -->
        <div v-if="listToolsReport.outdated.length > 0" class="outdated-strip" data-id="list-tools-outdated-alert">
          <n-icon :size="14"><WarningOutlined /></n-icon>
          {{ t('versionManagement.modals.listTools.outdated.title') }}:
          <span v-for="(o, i) in listToolsReport.outdated" :key="o.name">
            <strong>{{ o.name }}</strong> ({{ o.installed }} → {{ o.available }})<span v-if="i < listToolsReport.outdated.length - 1">, </span>
          </span>
        </div>

        <!-- Flat tools table -->
        <table class="tools-table" :data-id="`list-tools-table`">
          <thead>
            <tr>
              <th class="col-name">{{ t('versionManagement.modals.listTools.columns.tool') }}</th>
              <th class="col-desc">{{ t('versionManagement.modals.listTools.columns.description') }}</th>
              <th class="col-ver">{{ t('versionManagement.modals.listTools.columns.version') }}</th>
              <th class="col-status">{{ t('versionManagement.modals.listTools.columns.status') }}</th>
              <th class="col-inst">{{ t('versionManagement.modals.listTools.columns.installed') }}</th>
            </tr>
          </thead>
          <tbody>
            <template v-for="entry in listToolsReport.tools" :key="entry.tool.name">
              <tr
                v-for="vi in entry.version_inspections.filter(v => v.has_platform_download)"
                :key="vi.version.name"
                :data-id="`list-tools-tool-${entry.tool.name}-version-${vi.version.name}`"
              >
                <td class="tool-name-cell">
                  {{ entry.tool.name }}
                  <n-tag v-if="entry.tool.install === 'on_request'" size="tiny" type="info">
                    {{ t('versionManagement.modals.listTools.optional') }}
                  </n-tag>
                </td>
                <td class="desc-cell" :title="entry.tool.description">{{ entry.tool.description }}</td>
                <td class="ver-cell">{{ vi.version.name }}</td>
                <td>
                  <n-tag :type="statusTagType(vi.version.status)" size="small">{{ vi.version.status }}</n-tag>
                </td>
                <td>
                  <span v-if="vi.installed" class="installed-yes">
                    {{ vi.installed.version }}
                    <n-tag v-if="vi.installed.is_recommended_match" size="tiny" type="success">✓</n-tag>
                  </span>
                  <span v-else class="not-installed">—</span>
                </td>
              </tr>
            </template>
          </tbody>
        </table>

        <n-empty v-if="listToolsReport.tools.length === 0" :description="t('versionManagement.modals.listTools.empty')" data-id="list-tools-empty" />
      </div>
    </n-modal>

    <n-modal
      v-model:show="showListFeaturesModal"
      preset="card"
      :title="t('versionManagement.modals.listFeatures.title', { name: listFeaturesVersion?.name })"
      style="max-width: 900px;"
      :bordered="false"
      size="huge"
      data-id="list-features-modal"
    >
      <div v-if="listFeaturesLoading" class="list-tools-loading" data-id="list-features-loading">
        <n-spin :size="32" />
        <p>{{ t('versionManagement.modals.listFeatures.loading') }}</p>
      </div>

      <div v-else-if="listFeaturesReport" class="list-tools-content" data-id="list-features-content">
        <!-- Meta strip + add-features trigger, aligned on the same row -->
        <div class="list-tools-header">
          <div class="list-tools-meta">
            <div class="meta-row">
              <span class="meta-label">{{ t('versionManagement.modals.listFeatures.idfPath') }}:</span>
              <code class="meta-value">{{ listFeaturesReport.idf.path }}</code>
            </div>
            <div class="meta-row">
              <span class="meta-label">{{ t('versionManagement.modals.listFeatures.requirementsPath') }}:</span>
              <code class="meta-value">{{ listFeaturesReport.requirements_json_path }}</code>
            </div>
          </div>

          <n-button
            v-if="!showAddFeaturesPanel"
            size="small"
            type="primary"
            secondary
            @click="openAddFeaturesPanel"
            data-id="show-add-features-button"
          >
            <template #icon><n-icon><PlusCircleOutlined /></n-icon></template>
            {{ t('versionManagement.modals.listFeatures.addFeatures.button') }}
          </n-button>
        </div>

        <!-- Add more features panel -->
        <div v-if="showAddFeaturesPanel" class="add-tools-panel" data-id="add-features-panel">
          <h4>{{ t('versionManagement.modals.listFeatures.addFeatures.title') }}</h4>

          <n-empty
            v-if="availableFeaturesToAdd.length === 0"
            :description="t('versionManagement.modals.listFeatures.addFeatures.none')"
            size="small"
            data-id="add-features-none"
          />

          <n-checkbox-group v-else v-model:value="selectedExtraFeatures" data-id="add-features-checkbox-group">
            <n-space vertical>
              <n-checkbox
                v-for="entry in availableFeaturesToAdd"
                :key="entry.feature.name"
                :value="entry.feature.name"
                :data-id="`add-features-checkbox-${entry.feature.name}`"
              >
                <strong>{{ entry.feature.name }}</strong><span v-if="entry.feature.description"> — {{ entry.feature.description }}</span>
              </n-checkbox>
            </n-space>
          </n-checkbox-group>

          <div class="add-tools-actions">
            <n-button size="small" @click="cancelAddFeaturesPanel" data-id="add-features-cancel-button">
              {{ t('versionManagement.modals.listFeatures.addFeatures.cancel') }}
            </n-button>
            <n-button
              size="small"
              type="primary"
              :disabled="selectedExtraFeatures.length === 0"
              :loading="addingFeatures"
              @click="confirmAddFeatures"
              data-id="add-features-confirm-button"
            >
              {{ t('versionManagement.modals.listFeatures.addFeatures.confirm') }}
            </n-button>
          </div>
        </div>

        <!-- Flat features table -->
        <table class="tools-table" data-id="list-features-table">
          <thead>
            <tr>
              <th class="col-fname">{{ t('versionManagement.modals.listFeatures.columns.feature') }}</th>
              <th class="col-fdesc">{{ t('versionManagement.modals.listFeatures.columns.description') }}</th>
              <th class="col-fstatus">{{ t('versionManagement.modals.listFeatures.columns.status') }}</th>
              <th class="col-finst">{{ t('versionManagement.modals.listFeatures.columns.installed') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="entry in listFeaturesReport.features"
              :key="entry.feature.name"
              :data-id="`list-features-feature-${entry.feature.name}`"
            >
              <td class="tool-name-cell">
                {{ entry.feature.name }}
                <n-tag v-if="entry.feature.optional" size="tiny" type="info">
                  {{ t('versionManagement.modals.listFeatures.optional') }}
                </n-tag>
              </td>
              <td class="desc-cell" :title="entry.feature.description">{{ entry.feature.description }}</td>
              <td>
                <n-tag :type="entry.feature.optional ? 'info' : 'default'" size="small">
                  {{ entry.feature.optional
                    ? t('versionManagement.modals.listFeatures.optionalStatus')
                    : t('versionManagement.modals.listFeatures.requiredStatus') }}
                </n-tag>
              </td>
              <td>
                <span v-if="entry.installed" class="installed-yes">{{ t('versionManagement.modals.listFeatures.installedMarker') }}</span>
                <span v-else class="not-installed">—</span>
              </td>
            </tr>
          </tbody>
        </table>

        <n-empty v-if="listFeaturesReport.features.length === 0" :description="t('versionManagement.modals.listFeatures.empty')" data-id="list-features-empty" />
      </div>
    </n-modal>
  </div>
</template>

<script>
import { ref, computed, onMounted, onUnmounted, version } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { save } from '@tauri-apps/plugin-dialog'
import {
  NButton, NCard, NIcon, NTag, NEmpty, NModal, NInput,
  NCheckbox, NCheckboxGroup, NSpace, NAlert, NTooltip, NSpin, useMessage
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
  LaptopOutlined,
  SaveOutlined,
  UnorderedListOutlined,
  AppstoreOutlined
} from '@vicons/antd'
import { useAppStore } from '../store'

export default {
  name: 'VersionManagement',
  components: {
    NButton, NCard, NIcon, NTag, NEmpty, NModal, NInput,
    NCheckbox, NCheckboxGroup, NSpace, NAlert, NTooltip, NSpin,
    FolderOutlined, FolderOpenOutlined, EditOutlined,
    DeleteOutlined, ToolOutlined, PlusCircleOutlined,
    ClearOutlined, ReloadOutlined, UsbOutlined, LaptopOutlined,
    SaveOutlined, UnorderedListOutlined, AppstoreOutlined
  },
  setup() {
    const router = useRouter()
    const message = useMessage()
    const { t } = useI18n()

    const installedVersions = ref([])
    const os = ref('unknown')
    const prerequisitesInstalled = ref(true)
    const checkingUpdates = ref(false)

    // Event listener for prerequisites installation complete
    const unlistenInstallComplete = ref(null)

    // Modal states
    const showRenameModal = ref(false)
    const showRemoveModal = ref(false)
    const showFixModal = ref(false)
    const showPurgeModal = ref(false)
    const showListToolsModal = ref(false)
    const showListFeaturesModal = ref(false)
    const selectedVersion = ref(null)
    const newVersionName = ref('')
    const purgeConfirmed = ref(false)

    // List-tools modal state
    const listToolsVersion = ref(null)
    const listToolsReport = ref(null)
    const listToolsLoading = ref(false)
    const showAddToolsPanel = ref(false)
    const selectedExtraTools = ref([])
    const addingTools = ref(false)
    const appStore = useAppStore()

    // List-features modal state
    const listFeaturesVersion = ref(null)
    const listFeaturesReport = ref(null)
    const listFeaturesLoading = ref(false)
    const showAddFeaturesPanel = ref(false)
    const selectedExtraFeatures = ref([])
    const addingFeatures = ref(false)

    // Optional (on_request) tools that are not currently installed for this version -
    // candidates the user can pick to add on top of the existing fix/install.
    const availableToolsToAdd = computed(() => {
      if (!listToolsReport.value) return []
      return listToolsReport.value.tools.filter(entry =>
        entry.tool.install === 'on_request' &&
        entry.version_inspections.some(vi => vi.has_platform_download) &&
        !entry.version_inspections.some(vi => vi.installed)
      )
    })

    // Optional features that are not currently installed for this version -
    // candidates the user can pick to add on top of the existing fix/install.
    const availableFeaturesToAdd = computed(() => {
      if (!listFeaturesReport.value) return []
      return listFeaturesReport.value.features.filter(entry =>
        entry.feature.optional && !entry.installed
      )
    })

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
      os.value = await appStore.getOs();
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

    const renameVersion = (version) => {
      selectedVersion.value = version
      newVersionName.value = version.name
      showRenameModal.value = true
    }

    const openIDFTerminal = async (version) => {
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

    const openListTools = async (version) => {
      listToolsVersion.value = version
      listToolsReport.value = null
      listToolsLoading.value = true
      showListToolsModal.value = true
      showAddToolsPanel.value = false
      selectedExtraTools.value = []
      try {
        const report = await invoke('list_idf_tools', { id: version.id })
        if (!report) {
          message.error(t('versionManagement.messages.error.listTools'))
          showListToolsModal.value = false
          return
        }
        listToolsReport.value = report
      } catch (error) {
        console.error('Failed to list tools:', error)
        message.error(t('versionManagement.messages.error.listTools', { error }))
        showListToolsModal.value = false
      } finally {
        listToolsLoading.value = false
      }
    }

    const openAddToolsPanel = () => {
      selectedExtraTools.value = []
      showAddToolsPanel.value = true
    }

    const cancelAddToolsPanel = () => {
      showAddToolsPanel.value = false
      selectedExtraTools.value = []
    }

    const confirmAddTools = async () => {
      if (selectedExtraTools.value.length === 0) {
        return
      }
      addingTools.value = true
      try {
        const version = listToolsVersion.value
        await invoke('fix_installation', { id: version.id, extraTools: selectedExtraTools.value })

        message.success(t('versionManagement.messages.success.repairStarted'))
        showListToolsModal.value = false
        showAddToolsPanel.value = false

        // Navigate to installation progress with fix mode parameters, same as confirmFix
        router.push({
          path: '/installation-progress',
          query: {
            mode: 'fix',
            id: version.id,
            name: version.name,
            path: version.path,
            autotrack: 'true'
          }
        })
      } catch (error) {
        console.error('Add tools error:', error)
        message.error(t('versionManagement.messages.error.repair', { error }))
      } finally {
        addingTools.value = false
      }
    }

    const openListFeatures = async (version) => {
      listFeaturesVersion.value = version
      listFeaturesReport.value = null
      listFeaturesLoading.value = true
      showListFeaturesModal.value = true
      showAddFeaturesPanel.value = false
      selectedExtraFeatures.value = []
      try {
        const report = await invoke('list_idf_features', { id: version.id })
        if (!report) {
          message.error(t('versionManagement.messages.error.listFeatures'))
          showListFeaturesModal.value = false
          return
        }
        listFeaturesReport.value = report
      } catch (error) {
        console.error('Failed to list features:', error)
        message.error(t('versionManagement.messages.error.listFeatures', { error }))
        showListFeaturesModal.value = false
      } finally {
        listFeaturesLoading.value = false
      }
    }

    const openAddFeaturesPanel = () => {
      selectedExtraFeatures.value = []
      showAddFeaturesPanel.value = true
    }

    const cancelAddFeaturesPanel = () => {
      showAddFeaturesPanel.value = false
      selectedExtraFeatures.value = []
    }

    const confirmAddFeatures = async () => {
      if (selectedExtraFeatures.value.length === 0) {
        return
      }
      addingFeatures.value = true
      try {
        const version = listFeaturesVersion.value
        await invoke('fix_installation', { id: version.id, extraFeatures: selectedExtraFeatures.value })

        message.success(t('versionManagement.messages.success.repairStarted'))
        showListFeaturesModal.value = false
        showAddFeaturesPanel.value = false

        // Navigate to installation progress with fix mode parameters, same as confirmFix
        router.push({
          path: '/installation-progress',
          query: {
            mode: 'fix',
            id: version.id,
            name: version.name,
            path: version.path,
            autotrack: 'true'
          }
        })
      } catch (error) {
        console.error('Add features error:', error)
        message.error(t('versionManagement.messages.error.repair', { error }))
      } finally {
        addingFeatures.value = false
      }
    }

    const statusTagType = (status) => {
      switch (status) {
        case 'recommended':
          return 'success'
        case 'supported':
          return 'info'
        case 'deprecated':
          return 'warning'
        default:
          return 'default'
      }
    }

    const purgeAll = () => {
      purgeConfirmed.value = false
      showPurgeModal.value = true
    }

    const confirmPurge = async () => {
      if (!purgeConfirmed.value) {
        message.warning(t('versionManagement.messages.warning.confirmAction'))
        return false;
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
        // Recheck will happen automatically via event listener when installation completes
      } catch (error) {
        message.error(t('versionManagement.messages.error.prerequisites', { error }))
      }
    }

    // Recheck prerequisites status
    const recheckPrerequisites = async () => {
      try {
        prerequisitesInstalled.value = await invoke('check_prerequisites')
      } catch (error) {
        console.error('Failed to recheck prerequisites:', error)
      }
    }

    const installDrivers = async () => {
      try {
        let res = await invoke('check_elevated_permissions')
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

    const exportInstallationConfig = async (version) => {
      try {
        // Check if installationConfig is present
        if (!version.installationConfig) {
          message.error(t('versionManagement.messages.error.noConfigToExport'))
          return
        }

        // Get the config string from the tauri command
        const configContent = await invoke('generate_installation_config_for_version', {
          id: version.id
        })

        if (!configContent) {
          message.error(t('versionManagement.messages.error.exportConfig'))
          return
        }

        // Show save dialog
        const filePath = await save({
          defaultPath: 'config.toml',
          filters: [{
            name: 'TOML',
            extensions: ['toml']
          }]
        })

        if (filePath) {
          // Write the content to the file using tauri command
          await invoke('write_text_file', {
            path: filePath,
            content: configContent
          })
          message.success(t('versionManagement.messages.success.configExported'))
        }
      } catch (error) {
        console.error('Failed to export installation config:', error)
        message.error(t('versionManagement.messages.error.exportConfig'))
      }
    }

    const goToBasicInstaller = () => {
      router.push('/basic-installer')
    }

    onMounted(async () => {
      unlistenInstallComplete.value = await listen('prerequisites-install-complete', async () => {
        await recheckPrerequisites();
      });

      checkOS()
      loadInstalledVersions()
    })

    onUnmounted(() => {
      if (unlistenInstallComplete.value) {
        unlistenInstallComplete.value();
      }
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
      showListToolsModal,
      showListFeaturesModal,
      selectedVersion,
      newVersionName,
      purgeConfirmed,
      listToolsVersion,
      listToolsReport,
      listToolsLoading,
      showAddToolsPanel,
      selectedExtraTools,
      addingTools,
      availableToolsToAdd,
      listFeaturesVersion,
      listFeaturesReport,
      listFeaturesLoading,
      showAddFeaturesPanel,
      selectedExtraFeatures,
      addingFeatures,
      availableFeaturesToAdd,
      formatDate,
      formatSize,
      renameVersion,
      openIDFTerminal,
      confirmRename,
      removeVersion,
      confirmRemove,
      confirmFix,
      fixVersion,
      openInExplorer,
      openListTools,
      openAddToolsPanel,
      cancelAddToolsPanel,
      confirmAddTools,
      openListFeatures,
      openAddFeaturesPanel,
      cancelAddFeaturesPanel,
      confirmAddFeatures,
      statusTagType,
      purgeAll,
      confirmPurge,
      installPrerequisites,
      recheckPrerequisites,
      installDrivers,
      goToBasicInstaller,
      exportInstallationConfig,
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
  gap: 0.25rem;
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

.list-tools-content {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.outdated-strip {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
  padding: 6px 12px;
  font-size: 12px;
  background: #fefce8;
  color: #854d0e;
  border-top: 1px solid #fde68a;
  border-bottom: 1px solid #fde68a;
}

.tools-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
  table-layout: fixed;
}

.tools-table th {
  text-align: left;
  padding: 6px 10px;
  font-size: 11px;
  font-weight: 600;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  border-bottom: 1px solid #e5e7eb;
  background: #f9fafb;
  position: sticky;
  top: 0;
  z-index: 1;
}

.tools-table td {
  padding: 6px 10px;
  border-bottom: 1px solid #f3f4f6;
  vertical-align: middle;
}

.tools-table tbody tr:hover td { background: #f9fafb; }
.tools-table tbody tr:last-child td { border-bottom: none; }

.col-name  { width: 20%; }
.col-desc  { width: 35%; }
.col-ver   { width: 18%; }
.col-status{ width: 14%; }
.col-inst  { width: 13%; }

.col-fname   { width: 25%; }
.col-fdesc   { width: 45%; }
.col-fstatus { width: 15%; }
.col-finst   { width: 15%; }

.tool-name-cell { font-weight: 500; font-size: 13px; }
.desc-cell { color: #6b7280; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ver-cell  { font-family: monospace; font-size: 12px; color: #374151; }
.installed-yes { color: #16a34a; font-size: 12px; display: inline-flex; align-items: center; gap: 4px; }
.not-installed { color: #9ca3af; }

.list-tools-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 1rem;
  margin-bottom: 0.75rem;
}

.list-tools-header button {
  color:red
}

.list-tools-header .list-tools-meta {
  flex: 1;
  min-width: 0;
}

.add-tools-panel {
  margin-bottom: 1rem;
  padding: 0.75rem 1rem;
  border: 1px solid #93c5fd;
  border-radius: 6px;
  background: #eff6ff;
}

.add-tools-panel h4 {
  margin: 0 0 0.75rem 0;
  font-size: 0.95rem;
  color: #1f2937;
}

.add-tools-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
}
</style>
