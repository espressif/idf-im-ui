<template>
  <div class="tools-select" data-id="tools-select">
    <h1 class="title" data-id="tools-select-title">{{ t('toolsSelect.title') }}</h1>
    <p class="description" data-id="tools-select-description">{{ t('toolsSelect.description') }}</p>

    <n-card class="tools-card" data-id="tools-card">
      <n-spin :show="loading" data-id="tools-loading-spinner">
        <template v-if="!loading && versionTools.length > 0">
          <!-- Version Tabs (only show if multiple versions) -->
          <n-tabs
            v-if="versionTools.length > 1"
            v-model:value="activeVersion"
            type="line"
            class="version-tabs"
            data-id="version-tabs"
          >
            <n-tab-pane
              v-for="versionData in versionTools"
              :key="versionData.version"
              :name="versionData.version"
              :tab="versionData.version"
              :data-id="`version-tab-${versionData.version}`"
            >
              <tools-content
                :version="versionData.version"
                :required-tools="getRequiredTools(versionData.version)"
                :optional-tools="getOptionalTools(versionData.version)"
                :selected-tools="selectedToolsMap[versionData.version] || []"
                @toggle-tool="toggleTool"
                @select-all="selectAllOptional"
                @deselect-all="deselectAllOptional"
              />
            </n-tab-pane>
          </n-tabs>

          <!-- Single version (no tabs) -->
          <tools-content
            v-else
            :version="versionTools[0].version"
            :required-tools="getRequiredTools(versionTools[0].version)"
            :optional-tools="getOptionalTools(versionTools[0].version)"
            :selected-tools="selectedToolsMap[versionTools[0].version] || []"
            @toggle-tool="toggleTool"
            @select-all="selectAllOptional"
            @deselect-all="deselectAllOptional"
          />

          <div class="action-footer" data-id="tools-action-footer">
            <span class="selection-summary" data-id="selection-summary">
              {{ t('toolsSelect.summaryMultiVersion', {
                versions: versionTools.length,
                details: selectionSummary
              }) }}
            </span>
            <n-button
              @click="processChoices"
              type="error"
              size="large"
              :disabled="!canProceed"
              data-id="continue-tools-button"
            >
              {{ t('toolsSelect.continueButton') }}
            </n-button>
          </div>
        </template>

        <template v-else-if="!loading && versionTools.length === 0">
          <div class="empty-state" data-id="empty-state">
            <p class="empty-message">{{ t('toolsSelect.noTools') }}</p>
          </div>
        </template>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NTabs, NTabPane } from 'naive-ui';
import ToolsContent from './ToolsContent.vue';

export default {
  name: 'ToolsSelect',
  props: {
    nextstep: Function
  },
  components: {
    NButton,
    NSpin,
    NCard,
    NTabs,
    NTabPane,
    ToolsContent
  },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    loading: true,
    versionTools: [],
    selectedToolsMap: {},
    activeVersion: null,
  }),
  computed: {
    selectionSummary() {
      return this.versionTools.map(vt => {
        const selected = this.selectedToolsMap[vt.version]?.length || 0;
        const total = vt.tools.length;
        return `${vt.version}: ${selected}/${total}`;
      }).join(', ');
    },
    canProceed() {
      return this.versionTools.every(vt => {
        const required = this.getRequiredTools(vt.version);
        const selected = this.selectedToolsMap[vt.version] || [];
        return required.every(rt => selected.includes(rt.name));
      });
    }
  },
  methods: {
    async getAvailableTools() {
      try {
        this.loading = true;

        const versionTools = await invoke("get_tools_list_all_versions", {});
        this.versionTools = versionTools;

        // Initialize selected tools map
        const initialMap = {};
        for (const vt of versionTools) {
          const required = vt.tools.filter(t => t.install === 'always').map(t => t.name);
          initialMap[vt.version] = [...required];
        }
        this.selectedToolsMap = initialMap;

        if (versionTools.length > 0) {
          this.activeVersion = versionTools[0].version;
        }

        // Try to restore previously saved selections
        try {
          const savedTools = await invoke("get_selected_tools_per_version", {});
          if (savedTools && Object.keys(savedTools).length > 0) {
            for (const [version, tools] of Object.entries(savedTools)) {
              if (this.selectedToolsMap[version] !== undefined) {
                const required = this.getRequiredTools(version).map(t => t.name);
                const merged = [...new Set([...required, ...tools])];
                this.selectedToolsMap[version] = merged;
              }
            }
          } else {
            for (const vt of versionTools) {
              this.selectAllOptional(vt.version);
            }
          }
        } catch (err) {
          console.log("No previously saved tools per version");
          for (const vt of versionTools) {
            this.selectAllOptional(vt.version);
          }
        }

        this.loading = false;
      } catch (error) {
        console.error("Failed to load tools:", error);
        this.loading = false;
      }
    },

    getToolsForVersion(version) {
      const vt = this.versionTools.find(v => v.version === version);
      return vt ? vt.tools : [];
    },

    getRequiredTools(version) {
      return this.getToolsForVersion(version).filter(t => t.install === 'always');
    },

    getOptionalTools(version) {
      return this.getToolsForVersion(version).filter(t => t.install === 'on_request');
    },

    toggleTool(version, toolName) {
      const tool = this.getToolsForVersion(version).find(t => t.name === toolName);

      if (tool && tool.install === 'always') {
        return;
      }

      const selected = this.selectedToolsMap[version] || [];
      const index = selected.indexOf(toolName);

      if (index > -1) {
        selected.splice(index, 1);
      } else {
        selected.push(toolName);
      }

      this.selectedToolsMap = { ...this.selectedToolsMap, [version]: selected };
    },

    selectAllOptional(version) {
      const allToolNames = this.getToolsForVersion(version).map(t => t.name);
      this.selectedToolsMap = { ...this.selectedToolsMap, [version]: [...allToolNames] };
    },

    deselectAllOptional(version) {
      const required = this.getRequiredTools(version).map(t => t.name);
      this.selectedToolsMap = { ...this.selectedToolsMap, [version]: [...required] };
    },

    async processChoices() {
      console.log("Selected tools per version:", this.selectedToolsMap);

      if (!this.loading) {
        try {
          await invoke("set_selected_tools_per_version", {
            toolsMap: this.selectedToolsMap
          });
          this.nextstep();
        } catch (error) {
          console.error("Failed to save tools:", error);
        }
      }
    }
  },

  mounted() {
    this.getAvailableTools();
  }
}
</script>

<style scoped>
.tools-select {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.title {
  font-size: 27px;
  font-family: 'Trueno-bold', sans-serif;
  color: #374151;
  margin-bottom: 0.5rem;
}

.description {
  font-size: 21px;
  font-family: 'Trueno-light', sans-serif;
  color: #6b7280;
  margin-bottom: 2rem;
}

.tools-card {
  background: white;
  padding: 1.5rem;
}

.version-tabs {
  margin-bottom: 1rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 1rem;
  margin-top: 2rem;
  padding-top: 1rem;
}

.selection-summary {
  font-size: 0.875rem;
  color: #6b7280;
}

.empty-state {
  padding: 3rem;
  text-align: center;
}

.empty-message {
  font-size: 1rem;
  color: #6b7280;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;
  padding: 0px;
}

.n-card :deep(.n-card__content) {
  padding: 0px;
}

.n-button[type="primary"] {
  background-color: #E8362D;
  color: #e5e7eb;
}

.n-button {
  padding: 5px;
  background-color: #E8362D;
  color: #ffffff;
}
</style>
