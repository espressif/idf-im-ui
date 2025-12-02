<template>
  <div class="installation-progress" data-id="installation-progress">
    <h1 class="title" data-id="installation-title">
      {{ is_fix_mode ? t('installationProgress.title.repair') : t('installationProgress.title.installation') }}
    </h1>

    <n-alert :title="t('installationProgress.alert.error')" type="error" v-if="error_message">
      {{ error_message }}
    </n-alert>

    <n-card class="progress-card" data-id="progress-card">
      <div class="summary-section" data-id="installation-summary"
        v-if="!installation_running && !installation_finished && !installation_failed">

        <!-- Fix Mode Summary -->
        <div v-if="is_fix_mode" class="fix-info" data-id="fix-info">
          <h3 data-id="fix-title">{{ t('installationProgress.fixMode.title') }}</h3>
          <div class="fix-version-info">
            <div class="idf-version" v-if="fixing_version">
              {{ fixing_version.name || t('installationProgress.fixMode.unknownVersion') }}
            </div>
            <div class="fix-path" v-if="fixing_version">
              <strong>{{ t('installationProgress.fixMode.path') }}</strong> {{ fixing_version.path || t('installationProgress.fixMode.unknownPath') }}
            </div>
          </div>
          <div class="fix-description">
            <p>{{ t('installationProgress.fixMode.description') }}</p>
          </div>
        </div>

        <!-- Normal Install Mode Summary -->
        <div v-else class="versions-info" v-if="all_settings" data-id="versions-info">
          <h3 data-id="versions-title">{{ t('installationProgress.normalMode.title') }}</h3>
          <div class="version-chips" data-id="version-chips">
            <div v-for="version in idf_versions" :key="version" type="info" :data-id="`version-tag-${version}`"
              class="idf-version">
              {{ version }}
            </div>
          </div>
        </div>

        <!-- Start Installation Button - Only show when appropriate -->
        <div data-id="start-button-container" v-if="shouldShowStartButton()">
          <n-button @click="startInstallation()" type="error" size="large" :loading="installation_running"
            :disabled="installation_running" data-id="start-installation-button">
            {{ installation_running ? t('installationProgress.buttons.installing') : t('installationProgress.buttons.startInstallation') }}
          </n-button>
        </div>
      </div>

      <!-- Current Activity Display -->
      <div v-if="installation_running" class="current-activity" data-id="current-activity">
        <div class="current-step">
          <h3>{{ t('installationProgress.currentActivity.title') }}</h3>
          <div class="activity-status">{{ currentActivity }}</div>
          <div v-if="currentDetail" class="activity-detail">{{ currentDetail }}</div>
          <div v-if="installationPlan && installationPlan.total_versions > 1" class="multi-version-progress">
            <div class="version-overview">
              {{ t('installationProgress.currentActivity.installingVersions', { count: installationPlan.total_versions }) }}
              <span v-for="(version, index) in installationPlan.versions" :key="version"
                    class="version-indicator"
                    :class="{
                      'completed': completedVersions.includes(version),
                      'active': index === currentVersionIndex,
                      'pending': !completedVersions.includes(version) && index !== currentVersionIndex
                    }">
                {{ version }}
              </span>
            </div>
          </div>
        </div>

        <div class="progress-section">
          <div class="progress-label">{{ t('installationProgress.progress.overall') }}</div>
          <n-progress
            type="line"
            :percentage="currentProgress"
            :processing="installation_running"
            :indicator-placement="'inside'"
            color="#E8362D"
          />
        </div>

        <!-- Installation Steps -->
        <div class="installation-steps" v-if="installationSteps.length > 0">
          <div class="steps-container">
            <div
              v-for="(step, index) in installationSteps"
              :key="index"
              class="step-item"
              :class="{
                'active': index === currentStep,
                'completed': index < currentStep,
                'pending': index > currentStep
              }"
            >
              <div class="step-indicator">{{ index + 1 }}</div>
              <div class="step-content">
                <div class="step-title">{{ step.title }}</div>
                <div class="step-description">{{ step.description }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Error State -->
      <div v-if="installation_failed" class="error-message" data-id="error-message">
        <h3 data-id="error-title">{{ t('installationProgress.error.title', { mode: is_fix_mode ? t('installationProgress.title.repair').toLowerCase() : t('installationProgress.title.installation').toLowerCase() }) }}</h3>
        <p data-id="error-message-text">{{ error_message }} <br> {{ t('installationProgress.error.seeLog') }}</p>
        <n-button @click="goHome()" type="error" size="large" data-id="home-installation-button">{{ t('installationProgress.buttons.goBack') }}</n-button>
      </div>

      <!-- Completion Actions -->
      <div class="action-footer" v-if="installation_finished && !installation_failed" data-id="action-footer">
        <n-button @click="handleCompletion()" type="error" size="large" data-id="complete-installation-button-footer">
          {{ is_fix_mode ? t('installationProgress.buttons.completeRepair') : t('installationProgress.buttons.completeInstallation') }}
        </n-button>
      </div>

      <!-- Installation/Repair Summary -->
      <div v-if="installation_finished && !installation_failed" class="installation-summary"
        data-id="installation-summary">
        <h3>{{ is_fix_mode ? t('installationProgress.summary.repairComplete') : t('installationProgress.summary.installationComplete') }}</h3>
        <p>{{ is_fix_mode ? t('installationProgress.summary.repairDescription') : t('installationProgress.summary.installationDescription') }}</p>
        <div class="summary-details">
          <div v-if="installed_versions.length > 0">
            <strong>{{ is_fix_mode ? t('installationProgress.summary.repairedVersion') : t('installationProgress.summary.installedVersions') }}</strong> {{ installed_versions.join(', ') }}
          </div>
          <div v-if="installationPath">
            <strong>{{ t('installationProgress.summary.installationPath') }}</strong> {{ installationPath }}
          </div>
          <div v-if="completedToolsCount > 0">
            <strong>{{ is_fix_mode ? t('installationProgress.summary.toolsRepaired') : t('installationProgress.summary.toolsInstalled') }}</strong> {{ completedToolsCount }}
          </div>
        </div>
      </div>

      <!-- Installation Log -->
      <n-collapse arrow-placement="right" v-if="totalLogCount > 0">
        <n-collapse-item :title="t('installationProgress.log.title')" name="1">
          <template #header-extra>
            <span class="log-count">{{ t('installationProgress.log.entries', { count: totalLogCount }) }}</span>
          </template>

          <div class="log-container">
            <!-- Virtual scrolling container -->
            <div
              class="log-virtual-container"
              ref="virtualContainer"
              @scroll="onLogScroll"
            >
              <!-- Spacer for items above viewport -->
              <div
                class="virtual-spacer-top"
                :style="{ height: topSpacerHeight + 'px' }"
              ></div>

              <!-- Only render visible items -->
              <div class="log-scroll-container">
                <div
                  v-for="(message, index) in visibleLogs"
                  :key="`log-${startIndex + index}-${message.timestamp}`"
                  class="log-entry"
                  :style="{ height: itemHeight + 'px' }"
                >
                  <pre
                    class="log-message"
                    :class="getLogMessageClass(message)"
                    v-text="message.text"
                  ></pre>
                </div>
              </div>

              <!-- Spacer for items below viewport -->
              <div
                class="virtual-spacer-bottom"
                :style="{ height: bottomSpacerHeight + 'px' }"
              ></div>
            </div>
          </div>
        </n-collapse-item>
      </n-collapse>
    </n-card>
  </div>
</template>

<script>
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, NCollapse, NCollapseItem, NAlert, NProgress } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import { useWizardStore, useAppStore } from '../../store'
import { navigationState } from '../../router';
import { useI18n } from 'vue-i18n'

export default {
  name: 'InstallationProgress',
  props: {
    nextstep: Function,
    mode: {
      type: String,
      default: 'install' // 'install' or 'fix'
    },
    fixVersionId: {
      type: String,
      default: null
    }
  },
  components: {
    NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, NCollapse,
    NCollapseItem, NAlert, NProgress
  },

  setup() {
    const { t } = useI18n()
    return { t }
  },

  watch: {
    installation_running(newValue) {
      // Update the shared navigation state
      navigationState.setInstallationRunning(newValue);
    }
  },

  data() {
    return {
      os: undefined,
      all_settings: undefined,
      loading: true,
      tools: {},

      // Event listeners
      unlistenProgress: undefined,
      unlistenLog: undefined,
      unlistenPlan: undefined,

      // Installation state
      installation_running: false,
      installation_finished: false,
      installation_failed: false,
      error_message: "",

      // Progress tracking
      currentStep: 0,
      currentStage: 'checking',
      installationPlan: null,
      currentVersionIndex: 0,
      completedVersions: [],
      timeStarted: null,

      // Version tracking
      current_version: null,
      installed_versions: [],
      failed_versions: [],

      // Logging with Virtual scrolling
      visibleLogs: [],
      totalLogCount: 0,
      scrollTop: 0,
      containerHeight: 300,
      itemHeight: 24,
      visibleCount: 15,
      startIndex: 0,

      // UI state
      installationPath: "",
      completedToolsCount: 0,
      totalToolsCount: 0,
      showToolsTable: false,

      // progress tracking
      progressUpdateTrigger: 0,
      lastProgressUpdate: 0
    }
  },

  created() {
    this._allLogs = [];
    this.BUFFER_SIZE = 2;

    this._progressData = {
      currentProgress: 0,
      currentActivity: this.t('installationProgress.preparing'),
      currentDetail: "",
      lastUpdate: Date.now()
    };

    this._progressThrottle = null;

    // Installation steps - initialized with translations
    this.installationSteps = [
      { title: this.t('installationProgress.steps.check.title'), description: this.t('installationProgress.steps.check.description') },
      { title: this.t('installationProgress.steps.prerequisites.title'), description: this.t('installationProgress.steps.prerequisites.description') },
      { title: this.t('installationProgress.steps.download.title'), description: this.t('installationProgress.steps.download.description') },
      { title: this.t('installationProgress.steps.submodules.title'), description: this.t('installationProgress.steps.submodules.description') },
      { title: this.t('installationProgress.steps.tools.title'), description: this.t('installationProgress.steps.tools.description') },
      { title: this.t('installationProgress.steps.python.title'), description: this.t('installationProgress.steps.python.description') },
      { title: this.t('installationProgress.steps.configure.title'), description: this.t('installationProgress.steps.configure.description') },
      { title: this.t('installationProgress.steps.complete.title'), description: this.t('installationProgress.steps.complete.description') }
    ];
  },

  methods: {
    goHome: function () {
      this.store.resetWizard();
      this.$router.push('/');
    },

    startInstallation: async function () {
      this.installation_running = true;
      this.installation_finished = false;
      this.installation_failed = false;
      this.error_message = "";
      this.log_messages = [];

      this.clearLogs();

      this._progressData = {
        currentProgress: 0,
        currentActivity: this.t('installationProgress.preparing'),
        currentDetail: "",
        lastUpdate: Date.now()
      };

      this.currentStep = 0;
      this.currentStage = 'checking';
      this.forceProgressUpdate();
      this.timeStarted = new Date();

      try {
        const eventName = this.is_fix_mode ? "GUI fix installation started" : "GUI wizard installation started";
        await invoke("track_event_command", { name: eventName });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }

      try {
        if (this.is_fix_mode) {
          if (this.fixing_version) {
            this.current_version = this.fixing_version.name;
            this._progressData.currentActivity = `${this.t('installationProgress.fixMode.title')} ${this.fixing_version.name}...`;
          }
        } else {
          await invoke("start_installation_gui_cmd", {});
        }
      } catch (e) {
        console.error('Error during installation:', e);
        this.error_message = e.toString();
        this.installation_failed = true;
        this.installation_running = false;
      }
    },

    startListening: async function () {
      this.unlistenProgress = await listen('installation-progress', (event) => {
        this.handleProgressEvent(event.payload);
      });

      this.unlistenLog = await listen('log-message', (event) => {
        console.log('Log message received:', event.payload);
        this.handleLogMessage(event.payload);
      });

      this.unlistenPlan = await listen('installation-plan', (event) => {
        this.handleInstallationPlan(event.payload);
      });
    },

    handleInstallationPlan: function(plan) {
      this.installationPlan = plan;
      if (plan.current_version_index !== null && plan.current_version_index !== undefined) {
        this.currentVersionIndex = plan.current_version_index;
      }

      console.log('Installation plan received:', plan);
    },

    handleProgressEvent: function (payload) {
      const { stage, percentage, message, detail, version } = payload;
      const now = Date.now();

      this._progressData.currentProgress = percentage || 0;
      this._progressData.currentActivity = message || this._progressData.currentActivity;
      this._progressData.currentDetail = detail || "";
      this._progressData.lastUpdate = now;

      if (version && version !== this.current_version) {
        this.current_version = version;
      }

      if (stage !== this.currentStage) {
        this.currentStage = stage;
      }

      let newStep = this.currentStep;

      switch (stage) {
        case 'checking': newStep = 0; break;
        case 'prerequisites': newStep = 1; break;
        case 'download':
          newStep = 2;
          if (percentage > 10) newStep = 3;
          break;
        case 'extract': newStep = 3; break;
        case 'tools':
          newStep = 4;
          if (!this.showToolsTable) this.showToolsTable = true;
          break;
        case 'python': newStep = 5; break;
        case 'configure': newStep = 6; break;
        case 'complete':
          if (version && !this.completedVersions.includes(version)) {
            this.completedVersions.push(version);
          }
          newStep = 7;
          this.handleInstallationComplete(version);
          break;
        case 'error':
          this.handleInstallationError(message, detail);
          break;
      }

      if (newStep !== this.currentStep) {
        this.currentStep = newStep;
      }

      this.throttledProgressUpdate();
    },

    throttledProgressUpdate() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
      }

      this._progressThrottle = setTimeout(() => {
        const now = Date.now();
        if (now - this.lastProgressUpdate > 100) {
          this.progressUpdateTrigger++;
          this.lastProgressUpdate = now;
        }
        this._progressThrottle = null;
      }, 100);
    },

    forceProgressUpdate() {
      this.progressUpdateTrigger++;
      this.lastProgressUpdate = Date.now();
    },

    handleLogMessage: function (payload) {
      const { level, message } = payload;

      const logEntry = {
        level,
        text: message,
        timestamp: Date.now(),
        id: this._allLogs.length
      };

      this._allLogs.unshift(logEntry);

      const MAX_LOG_ENTRIES = 1000;
      if (this._allLogs.length > MAX_LOG_ENTRIES) {
        this._allLogs = this._allLogs.slice(0, MAX_LOG_ENTRIES);
      }

      this.totalLogCount = this._allLogs.length;
      this.updateVisibleLogs();

      if (this.scrollTop < this.itemHeight) {
        this.scrollToTop();
      }

      if (message.includes('installed at:') || message.includes('Location:')) {
        const pathMatch = message.match(/(?:installed at:|Location:)\s*(.+)/i);
        if (pathMatch && pathMatch[1]) {
          this.installationPath = pathMatch[1].trim();
        }
      }
    },

    updateVisibleLogs() {
      const startIndex = Math.max(0, Math.floor(this.scrollTop / this.itemHeight) - this.BUFFER_SIZE);
      const endIndex = Math.min(
        startIndex + this.maxVisibleItems,
        this._allLogs.length
      );

      this.startIndex = startIndex;

      this.visibleLogs = this._allLogs.slice(startIndex, endIndex).map(log => ({
        ...log
      }));
    },

    onLogScroll(event) {
      const newScrollTop = event.target.scrollTop;

      if (Math.abs(newScrollTop - this.scrollTop) > this.itemHeight / 2) {
        this.scrollTop = newScrollTop;
        this.updateVisibleLogs();
      }
    },

    scrollToTop() {
      this.$nextTick(() => {
        const container = this.$refs.virtualContainer;
        if (container) {
          container.scrollTop = 0;
          this.scrollTop = 0;
          this.updateVisibleLogs();
        }
      });
    },

    clearLogs: function() {
      this._allLogs = [];
      this.visibleLogs = [];
      this.totalLogCount = 0;
      this.scrollTop = 0;
      this.startIndex = 0;

      this.$forceUpdate();
    },

    measureContainer() {
      this.$nextTick(() => {
        const container = this.$refs.virtualContainer;
        if (container) {
          this.containerHeight = container.clientHeight;
          this.updateVisibleLogs();
        }
      });
    },

    getLogMessageClass: function (message) {
      if (message.level === 'error') return 'log-message log-error';
      if (message.level === 'warning') return 'log-message log-warning';
      if (message.level === 'success') return 'log-message log-success';
      if (message.text && (message.text.includes('WARN') || message.text.includes('ERR'))) {
        return 'log-message highlight';
      }
      return 'log-message';
    },

    handleToolsProgress: function (message, detail, percentage) {
      if (!this.current_version) return;

      if (!this.tools[this.current_version]) {
        this.tools[this.current_version] = {};
      }

      let toolName = null;
      if (message.includes('Installing:') || message.includes('Downloading:') || message.includes('Extracting:')) {
        const match = message.match(/(?:Installing:|Downloading:|Extracting:|Installed:)\s*(.+)/);
        if (match && match[1]) {
          toolName = match[1].trim();
        }
      }

      if (toolName) {
        if (!this.tools[this.current_version][toolName]) {
          this.tools[this.current_version][toolName] = {
            displayName: toolName,
            status: 'pending',
            progress: 0
          };
          this.totalToolsCount++;
        }

        const tool = this.tools[this.current_version][toolName];

        if (message.includes('Downloading:') || message.includes('Preparing:')) {
          tool.status = 'downloading';
        } else if (message.includes('Verifying:')) {
          tool.status = 'verifying';
        } else if (message.includes('Extracting:')) {
          tool.status = 'extracting';
        } else if (message.includes('Installed:')) {
          tool.status = 'completed';
          tool.progress = 100;
          this.completedToolsCount++;
        }

        if (detail) {
          const progressMatch = detail.match(/(\d+)%/);
          if (progressMatch) {
            tool.progress = parseInt(progressMatch[1]);
          }
        }
      }
    },

    handleInstallationComplete: function (version) {
      this.installation_running = false;
      this.installation_finished = true;
      this.currentProgress = 100;

      if (version && !this.installed_versions.includes(version)) {
        this.installed_versions.push(version);
      }

      if (this.current_version && !this.installed_versions.includes(this.current_version)) {
        this.installed_versions.push(this.current_version);
      }

      try {
        const eventName = this.is_fix_mode ? "GUI fix installation succeeded" : "GUI wizard installation succeeded";
        invoke("track_event_command", {
          name: eventName,
          additional_data: {
            duration_seconds: (new Date() - this.timeStarted) / 1000,
            version: version || this.current_version
          }
        });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }
    },

    handleInstallationError: function (message, detail) {
      this.installation_running = false;
      this.installation_failed = true;
      this.error_message = message || "Installation failed";

      if (this.current_version && !this.failed_versions.includes(this.current_version)) {
        this.failed_versions.push(this.current_version);
      }

      try {
        const eventName = this.is_fix_mode ? "GUI fix installation failed" : "GUI wizard installation failed";
        invoke("track_event_command", {
          name: eventName,
          additional_data: {
            duration_seconds: (new Date() - this.timeStarted) / 1000,
            version: this.current_version,
            errorMessage: message,
            errorDetails: detail
          }
        });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }
    },

    getToolStatusText: function (status) {
      const statusMap = {
        'pending': 'Waiting',
        'downloading': 'Downloading',
        'verifying': 'Verifying',
        'extracting': 'Extracting',
        'completed': 'Completed',
        'error': 'Error'
      };
      return statusMap[status] || status;
    },

    getToolStatusClass: function (status) {
      const classMap = {
        'pending': 'tool-status-pending',
        'downloading': 'tool-status-active',
        'verifying': 'tool-status-active',
        'extracting': 'tool-status-active',
        'completed': 'tool-status-success',
        'error': 'tool-status-error'
      };
      return classMap[status] || '';
    },

    get_settings: async function () {
      this.all_settings = await invoke("get_settings", {});
      if (this.all_settings && this.all_settings.path) {
        this.installationPath = this.all_settings.path;
      }
    },

    get_os: async function () {
      this.os = await this.appStore.getOs();
    },

    handleCompletion() {
      if (this.nextstep && typeof this.nextstep === 'function') {
        this.nextstep();
      } else {
        this.handleDirectNavigation();
      }
    },

    handleDirectNavigation() {
      if (this.is_fix_mode) {
        this.$router.push({
          path: '/version-management',
          query: {
            fixed: this.fixing_version?.id,
            message: `Successfully repaired ESP-IDF ${this.fixing_version?.name}`
          }
        });
      } else {
        this.$router.push({
          path: '/welcome',
          query: {
            installed: 'true',
            versions: this.installed_versions.join(','),
            message: `Successfully installed ESP-IDF versions: ${this.installed_versions.join(', ')}`
          }
        });
      }
    },

    shouldShowStartButton() {
      return !this.is_fix_mode &&
            !this.installation_running &&
            !this.installation_failed &&
            !this.installation_finished;
    },

    cleanupProgressData() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
        this._progressThrottle = null;
      }
      this._progressData = null;
    },

    cleanup: function() {
      this.clearLogs();
      this.cleanupProgressData();

      window.removeEventListener('resize', this.measureContainer);

      if (this.unlistenProgress) {
        this.unlistenProgress();
        this.unlistenProgress = null;
      }
      if (this.unlistenLog) {
        this.unlistenLog();
        this.unlistenLog = null;
      }
      if (this.unlistenPlan) {
        this.unlistenPlan();
        this.unlistenPlan = null;
      }

      this._allLogs = null;
    },
  },

  computed: {
    store() {
      return useWizardStore()
    },

    appStore() {
      return useAppStore()
    },

    idf_versions() {
      return this.all_settings ? this.all_settings.idf_versions : [];
    },

    tools_tabs() {
      return [...new Set([
        ...this.installed_versions,
        ...this.failed_versions,
        ...(this.current_version ? [this.current_version] : [])
      ])];
    },

    is_fix_mode() {
      return this.mode === 'fix' || this.$route.query.mode === 'fix';
    },

    fixing_version() {
      if (this.is_fix_mode) {
        return {
          id: this.$route.query.id || this.fixVersionId,
          name: this.$route.query.name || this.t('installationProgress.fixMode.unknownVersion'),
          path: this.$route.query.path || this.t('installationProgress.fixMode.unknownPath')
        };
      }
      return null;
    },

    topSpacerHeight() {
      return this.startIndex * this.itemHeight;
    },

    bottomSpacerHeight() {
      const remainingItems = Math.max(0, this.totalLogCount - (this.startIndex + this.visibleLogs.length));
      return remainingItems * this.itemHeight;
    },

    maxVisibleItems() {
      return Math.ceil(this.containerHeight / this.itemHeight) + (this.BUFFER_SIZE * 2);
    },

    currentProgress() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentProgress : 0;
    },

    currentActivity() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentActivity : this.t('installationProgress.preparing');
    },

    currentDetail() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentDetail : "";
    }
  },

  mounted() {
    this.get_os();
    this.get_settings();
    this.startListening();
    this.measureContainer();
    window.addEventListener('resize', this.measureContainer);
    navigationState.setInstallationRunning(this.installation_running);

    if (this.is_fix_mode && this.$route.query.mode === 'fix') {
      this.installation_running = true;
      if (this.fixing_version) {
        this.current_version = this.fixing_version.name;
        this.currentActivity = `${this.t('installationProgress.fixMode.title')} ${this.fixing_version.name}...`;
      }
    } else if (this.$route.query.autostart === 'true') {
      this.startInstallation();
    }

    if (this.$route.query.message) {
      console.log('Route message:', this.$route.query.message);
    }
  },

  beforeDestroy() {
    this.cleanup();
  },

  beforeUnmount() {
    navigationState.setInstallationRunning(false);
    this.cleanup();
  },
}
</script>

<style scoped>
.installation-progress {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.progress-card {
  background: white;
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  align-content: center;
}

.summary-section {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  margin-bottom: 2rem;
}

.versions-info h3 {
  font-size: 1.1rem;
  color: #374151;
  margin-bottom: 1rem;
}

.version-chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
}

.idf-version {
  border: 1px solid #428ED2;
  border-radius: 4px;
  width: 124px;
  height: 40px;
  display: flex;
  justify-content: center;
  align-items: center;
}

.current-activity {
  margin: 1rem 0;
  padding: 1rem;
  background-color: #f9fafb;
  border-radius: 8px;
  border-left: 4px solid #428ED2;
}

.current-step h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1rem;
  color: #6b7280;
}

.activity-status {
  font-size: 1.1rem;
  font-weight: 500;
  color: #374151;
}

.activity-detail {
  font-size: 0.9rem;
  color: #6b7280;
  margin-top: 0.5rem;
}

.progress-section {
  margin-top: 1rem;
}

.progress-label {
  font-size: 0.875rem;
  color: #6b7280;
  margin-bottom: 0.5rem;
}

.installation-steps {
  margin-top: 1.5rem;
}

.steps-container {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 1rem;
}

.step-item {
  display: flex;
  align-items: center;
  padding: 0.75rem;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  transition: all 0.2s ease;
}

.step-item.active {
  border-color: #428ED2;
  background-color: #eff6ff;
}

.step-item.completed {
  border-color: #10b981;
  background-color: #f0fdf4;
}

.step-item.completed .step-indicator {
  background-color: #10b981;
  color: white;
}

.step-item.active .step-indicator {
  background-color: #428ED2;
  color: white;
}

.step-indicator {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background-color: #e5e7eb;
  color: #6b7280;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: bold;
  margin-right: 0.75rem;
}

.step-content {
  flex: 1;
}

.step-title {
  font-weight: 500;
  color: #374151;
  font-size: 0.9rem;
}

.step-description {
  font-size: 0.8rem;
  color: #6b7280;
  margin-top: 0.25rem;
}

.tools-section {
  margin-top: 1rem;
}

.tools-tabs {
  margin-top: 1rem;
}

.tool-progress {
  font-weight: 500;
}

.tool-status-pending {
  color: #6b7280;
}

.tool-status-active {
  color: #428ED2;
  font-weight: 500;
}

.tool-status-success {
  color: #10b981;
  font-weight: 500;
}

.tool-status-error {
  color: #ef4444;
  font-weight: 500;
}

.error-message {
  margin-top: 1rem;
  border: 1px dotted #E8362D;
  padding: 1rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  margin-bottom: 1rem;
}

.installation-summary {
  margin: 1.5rem 0;
  padding: 1.5rem;
  border-radius: 8px;
  background-color: #f0f9ff;
  border: 1px solid #bfdbfe;
}

.summary-details {
  margin-top: 1rem;
  display: grid;
  gap: 0.5rem;
}

.log-container {
  text-align: left;
  background-color: white;
}

.log-virtual-container {
  height: 300px;
  overflow-y: auto;
  overflow-x: hidden;
  will-change: scroll-position;
  -webkit-overflow-scrolling: touch;
  scroll-behavior: smooth;
}

.virtual-spacer-top,
.virtual-spacer-bottom {
  width: 100%;
  pointer-events: none;
}

.log-scroll-container {
  contain: layout style;
}

.log-entry {
  height: 24px;
  display: flex;
  align-items: flex-start;
  contain: layout;
  box-sizing: border-box;
}

.log-message {
  margin: 0;
  padding: 2px 4px;
  font-family: monospace;
  font-size: 0.85rem;
  line-height: 20px;
  text-rendering: optimizeSpeed;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  width: 100%;
  flex: 1;
}

.log-message.log-error {
  background-color: #fee2e2;
  color: #b91c1c;
  border-left: 3px solid #ef4444;
}

.log-message.log-warning {
  background-color: #fef3c7;
  color: #d97706;
  border-left: 3px solid #f59e0b;
}

.log-message.log-success {
  color: #059669;
  border-left: 3px solid #10b981;
}

.log-message.highlight {
  background-color: #fff9c2;
  font-weight: 500;
  border-left: 3px solid #E8362D;
}

.log-count {
  font-size: 0.8rem;
  color: #6b7280;
  font-weight: normal;
}

.log-virtual-container::-webkit-scrollbar {
  width: 8px;
}

.log-virtual-container::-webkit-scrollbar-track {
  background: #f1f1f1;
  border-radius: 4px;
}

.log-virtual-container::-webkit-scrollbar-thumb {
  background: #c1c1c1;
  border-radius: 4px;
}

.log-virtual-container::-webkit-scrollbar-thumb:hover {
  background: #a1a1a1;
}

.log-virtual-container * {
  backface-visibility: hidden;
}

@media (max-width: 768px) {
  .log-virtual-container {
    height: 250px;
  }

  .log-entry {
    height: 28px;
  }

  .log-message {
    line-height: 24px;
  }
}

.fix-info {
  text-align: center;
}

.fix-info h3 {
  font-size: 1.1rem;
  color: #374151;
  margin-bottom: 1rem;
}

.fix-version-info {
  margin: 1rem 0;
}

.fix-path {
  margin-top: 0.5rem;
  font-size: 0.9rem;
  color: #6b7280;
}

.fix-description {
  margin-top: 1rem;
  padding: 1rem;
  background-color: #fffbeb;
  border: 1px solid #fbbf24;
  border-radius: 6px;
}

.fix-description p {
  margin: 0;
  color: #92400e;
  font-size: 0.9rem;
}

.n-button {
  background: #E8362D;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;
  padding-top: 0px;
}

.n-collapse {
  background-color: #FAFAFA;
  border: 1px solid #D5D5D5;
  max-height: 300px;
  overflow: auto;
}

tbody span {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 20px;
  color: #428ED2
}

tr > td {
  text-align: center;
}

tr > td:first-child {
  text-align: left;
}

.n-tab-pane {
  max-height: 300px;
  overflow-y: auto;
}

.multi-version-progress {
  margin-top: 1rem;
  padding: 0.75rem;
  background-color: #f8fafc;
  border-radius: 6px;
  border: 1px solid #e2e8f0;
}

.version-overview {
  font-size: 0.9rem;
  color: #64748b;
}

.version-indicator {
  display: inline-block;
  padding: 0.25rem 0.5rem;
  margin: 0.25rem;
  border-radius: 4px;
  font-size: 0.8rem;
  font-weight: 500;
}

.version-indicator.completed {
  background-color: #dcfce7;
  color: #166534;
  border: 1px solid #bbf7d0;
}

.version-indicator.active {
  background-color: #dbeafe;
  color: #1e40af;
  border: 1px solid #93c5fd;
}

.version-indicator.pending {
  background-color: #f1f5f9;
  color: #64748b;
  border: 1px solid #cbd5e1;
}
</style>
