<template>
  <div class="tools-select" data-id="tools-select">
    <h1 class="title" data-id="tools-select-title">{{ t('toolsSelect.title') }}</h1>
    <p class="description" data-id="tools-select-description">{{ t('toolsSelect.description') }}</p>

    <n-card class="tools-card" data-id="tools-card">
      <n-spin :show="loading" data-id="tools-loading-spinner">
        <template v-if="!loading && versionTools.length > 0">
          <!-- Version Tabs -->
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
              <div class="tools-content" data-id="tools-content">
                <div class="tools-sections" data-id="tools-sections">

                  <!-- Required Tools Section -->
                  <div class="tool-section" data-id="required-section">
                    <div class="section-header">
                      <h3 class="section-title" data-id="required-title">
                        {{ t('toolsSelect.sections.required') }}
                      </h3>
                      <span class="tool-count">{{ getRequiredTools(versionData.version).length }}</span>
                    </div>
                    <div class="tool-group" data-id="required-group">
                      <div
                        v-for="tool in getRequiredTools(versionData.version)"
                        :key="`${versionData.version}-${tool.name}`"
                        class="tool-row required"
                        :data-id="`tool-row-${versionData.version}-${tool.name}`"
                      >
                        <div class="tool-checkbox-wrapper">
                          <n-checkbox
                            :checked="true"
                            disabled
                            :data-id="`tool-checkbox-${versionData.version}-${tool.name}`"
                          />
                        </div>
                        <div class="tool-info">
                          <span class="tool-name" :data-id="`tool-name-${versionData.version}-${tool.name}`">
                            {{ tool.name }}
                          </span>
                          <span
                            v-if="tool.description"
                            class="tool-desc"
                            :data-id="`tool-desc-${versionData.version}-${tool.name}`"
                          >
                            {{ tool.description }}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  <!-- Optional Tools Section -->
                  <div class="tool-section" data-id="optional-section">
                    <div class="section-header">
                      <h3 class="section-title" data-id="optional-title">
                        {{ t('toolsSelect.sections.optional') }}
                      </h3>
                      <div class="section-actions">
                        <n-button
                          @click="selectAllOptional(versionData.version)"
                          size="small"
                          text
                          type="primary"
                          data-id="select-all-button"
                        >
                          {{ t('toolsSelect.actions.selectAll') }}
                        </n-button>
                        <span class="divider">|</span>
                        <n-button
                          @click="deselectAllOptional(versionData.version)"
                          size="small"
                          text
                          type="primary"
                          data-id="deselect-all-button"
                        >
                          {{ t('toolsSelect.actions.deselectAll') }}
                        </n-button>
                      </div>
                    </div>
                    <div class="tool-group" data-id="optional-group">
                      <div
                        v-for="tool in getOptionalTools(versionData.version)"
                        :key="`${versionData.version}-${tool.name}`"
                        class="tool-row optional"
                        :class="{ 'selected': isToolSelected(versionData.version, tool.name) }"
                        :data-id="`tool-row-${versionData.version}-${tool.name}`"
                        @click="toggleTool(versionData.version, tool.name)"
                      >
                        <div class="tool-checkbox-wrapper">
                          <n-checkbox
                            :checked="isToolSelected(versionData.version, tool.name)"
                            :data-id="`tool-checkbox-${versionData.version}-${tool.name}`"
                            @update:checked="() => toggleTool(versionData.version, tool.name)"
                          />
                        </div>
                        <div class="tool-info">
                          <span class="tool-name" :data-id="`tool-name-${versionData.version}-${tool.name}`">
                            {{ tool.name }}
                          </span>
                          <span
                            v-if="tool.description"
                            class="tool-desc"
                            :data-id="`tool-desc-${versionData.version}-${tool.name}`"
                          >
                            {{ tool.description }}
                          </span>
                        </div>
                      </div>
                      <div v-if="getOptionalTools(versionData.version).length === 0" class="no-optional-tools">
                        {{ t('toolsSelect.noOptionalTools') }}
                      </div>
                    </div>
                  </div>

                </div>
              </div>
            </n-tab-pane>
          </n-tabs>

          <!-- Single version (no tabs needed) -->
          <template v-else>
            <div class="tools-content" data-id="tools-content">
              <div class="tools-sections" data-id="tools-sections">

                <!-- Required Tools Section -->
                <div class="tool-section" data-id="required-section">
                  <div class="section-header">
                    <h3 class="section-title" data-id="required-title">
                      {{ t('toolsSelect.sections.required') }}
                    </h3>
                    <span class="tool-count">{{ getRequiredTools(versionTools[0].version).length }}</span>
                  </div>
                  <div class="tool-group" data-id="required-group">
                    <div
                      v-for="tool in getRequiredTools(versionTools[0].version)"
                      :key="tool.name"
                      class="tool-row required"
                      :data-id="`tool-row-${tool.name}`"
                    >
                      <div class="tool-checkbox-wrapper">
                        <n-checkbox
                          :checked="true"
                          disabled
                          :data-id="`tool-checkbox-${tool.name}`"
                        />
                      </div>
                      <div class="tool-info">
                        <span class="tool-name" :data-id="`tool-name-${tool.name}`">
                          {{ tool.name }}
                        </span>
                        <span
                          v-if="tool.description"
                          class="tool-desc"
                          :data-id="`tool-desc-${tool.name}`"
                        >
                          {{ tool.description }}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- Optional Tools Section -->
                <div class="tool-section" data-id="optional-section">
                  <div class="section-header">
                    <h3 class="section-title" data-id="optional-title">
                      {{ t('toolsSelect.sections.optional') }}
                    </h3>
                    <div class="section-actions">
                      <n-button
                        @click="selectAllOptional(versionTools[0].version)"
                        size="small"
                        text
                        type="primary"
                        data-id="select-all-button"
                      >
                        {{ t('toolsSelect.actions.selectAll') }}
                      </n-button>
                      <span class="divider">|</span>
                      <n-button
                        @click="deselectAllOptional(versionTools[0].version)"
                        size="small"
                        text
                        type="primary"
                        data-id="deselect-all-button"
                      >
                        {{ t('toolsSelect.actions.deselectAll') }}
                      </n-button>
                    </div>
                  </div>
                  <div class="tool-group" data-id="optional-group">
                    <div
                      v-for="tool in getOptionalTools(versionTools[0].version)"
                      :key="tool.name"
                      class="tool-row optional"
                      :class="{ 'selected': isToolSelected(versionTools[0].version, tool.name) }"
                      :data-id="`tool-row-${tool.name}`"
                      @click="toggleTool(versionTools[0].version, tool.name)"
                    >
                      <div class="tool-checkbox-wrapper">
                        <n-checkbox
                          :checked="isToolSelected(versionTools[0].version, tool.name)"
                          :data-id="`tool-checkbox-${tool.name}`"
                          @update:checked="() => toggleTool(versionTools[0].version, tool.name)"
                        />
                      </div>
                      <div class="tool-info">
                        <span class="tool-name" :data-id="`tool-name-${tool.name}`">
                          {{ tool.name }}
                        </span>
                        <span
                          v-if="tool.description"
                          class="tool-desc"
                          :data-id="`tool-desc-${tool.name}`"
                        >
                          {{ tool.description }}
                        </span>
                      </div>
                    </div>
                    <div v-if="getOptionalTools(versionTools[0].version).length === 0" class="no-optional-tools">
                      {{ t('toolsSelect.noOptionalTools') }}
                    </div>
                  </div>
                </div>

              </div>
            </div>
          </template>

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
import { ref, computed } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NCheckbox, NTabs, NTabPane } from 'naive-ui'

export default {
  name: 'ToolsSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NCheckbox, NTabs, NTabPane },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    loading: true,
    // Array of { version: string, tools: ToolSelectionInfo[] }
    versionTools: [],
    // Map of version -> selected tool names
    selectedToolsMap: {},
    // Currently active tab
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
      // Check that each version has at least the required tools selected
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

        // Fetch tools for all versions
        const versionTools = await invoke("get_tools_list_all_versions", {});
        this.versionTools = versionTools;

        // Initialize selected tools map with required tools for each version
        const initialMap = {};
        for (const vt of versionTools) {
          const required = vt.tools.filter(t => t.install === 'always').map(t => t.name);
          initialMap[vt.version] = [...required];
        }
        this.selectedToolsMap = initialMap;

        // Set active tab to first version
        if (versionTools.length > 0) {
          this.activeVersion = versionTools[0].version;
        }

        // Try to restore previously saved selections
        try {
          const savedTools = await invoke("get_selected_tools_per_version", {});
          if (savedTools && Object.keys(savedTools).length > 0) {
            // Merge saved tools with required tools
            for (const [version, tools] of Object.entries(savedTools)) {
              if (this.selectedToolsMap[version] !== undefined) {
                const required = this.getRequiredTools(version).map(t => t.name);
                // Ensure required tools are always included
                const merged = [...new Set([...required, ...tools])];
                this.selectedToolsMap[version] = merged;
              }
            }
          }
        } catch (err) {
          console.log("No previously saved tools per version");
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

    isToolSelected(version, toolName) {
      const selected = this.selectedToolsMap[version] || [];
      return selected.includes(toolName);
    },

    toggleTool(version, toolName) {
      const tool = this.getToolsForVersion(version).find(t => t.name === toolName);

      // Don't allow toggling required tools
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

      // Trigger reactivity
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

.tools-content {
  margin-bottom: 1.5rem;
}

.tools-sections {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.tool-section {
  display: flex;
  flex-direction: column;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid #e5e7eb;
}

.section-title {
  font-size: 1rem;
  font-weight: 600;
  color: #374151;
  margin: 0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.tool-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.5rem;
  height: 1.5rem;
  padding: 0 0.375rem;
  background-color: #e5e7eb;
  color: #6b7280;
  border-radius: 0.75rem;
  font-size: 0.75rem;
  font-weight: 500;
}

.section-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
}

.divider {
  color: #d1d5db;
}

.tool-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-height: 300px;
  overflow-y: auto;
}

.tool-row {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.625rem 0.75rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.375rem;
  background: white;
  transition: all 0.2s ease;
}

.tool-row.optional {
  cursor: pointer;
}

.tool-row.optional:hover {
  border-color: #e7352c;
  background-color: #fef2f2;
}

.tool-row.optional.selected {
  background-color: #fee2e2;
  border-color: #e7352c;
}

.tool-row.required {
  background-color: #f0f9ff;
  border-color: #bfdbfe;
  cursor: default;
}

.tool-checkbox-wrapper {
  display: flex;
  align-items: center;
  padding-top: 0.125rem;
}

.tool-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  pointer-events: none;
}

.tool-name {
  font-size: 0.875rem;
  font-weight: 500;
  color: #374151;
}

.tool-desc {
  font-size: 0.8125rem;
  color: #6b7280;
  line-height: 1.4;
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

.no-optional-tools {
  padding: 1rem;
  text-align: center;
  color: #6b7280;
  font-style: italic;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;
  padding: 0px;
}

.n-card__content {
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
