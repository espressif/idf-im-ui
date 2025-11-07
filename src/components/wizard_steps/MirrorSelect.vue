<template>
  <div class="mirror-select" data-id="mirror-select">
    <h1 class="title" data-id="mirror-select-title">{{ t('mirrorSelect.title') }}</h1>
    <p class="description" data-id="mirror-select-description">{{ t('mirrorSelect.description') }}</p>

    <n-card class="mirrors-card" data-id="mirrors-card">
      <n-spin :show="loading_idfs || loading_tools || loading_pypi" data-id="mirrors-loading-spinner">
        <div class="mirrors-grid" data-id="mirrors-grid">
          <!-- IDF Mirror Selection -->
          <div class="mirror-section" data-id="idf-mirror-section">
            <h3 class="section-title" data-id="idf-section-title">{{ t('mirrorSelect.sections.idfMirror') }}</h3>
            <n-radio-group v-model:value="selected_idf_mirror" class="mirror-options" data-id="idf-mirror-radio-group">
              <div v-for="mirror in idf_mirrors" :key="mirror.value" class="mirror-option"
                :class="{ 'selected': selected_idf_mirror === mirror.value }"
                :data-id="`idf-mirror-option-${mirror.value}`"
                @click="selected_idf_mirror = mirror.value">
                <n-radio :value="mirror.value" :data-id="`idf-mirror-radio-${mirror.value}`">
                  <div class="mirror-content" :data-id="`idf-mirror-content-${mirror.value}`">
                    <span class="mirror-url" :data-id="`idf-mirror-url-${mirror.value}`">{{ mirror.label }}</span>
                    <span v-if="isDefaultMirror(mirror.value, 'idf')" class="mirror-tag"
                      :data-id="`idf-mirror-default-tag-${mirror.value}`">{{ t('mirrorSelect.tags.default') }}</span>
                  </div>
                </n-radio>
              </div>
            </n-radio-group>
          </div>

          <!-- Tools Mirror Selection -->
          <div class="mirror-section" data-id="tools-mirror-section">
            <h3 class="section-title" data-id="tools-section-title">{{ t('mirrorSelect.sections.toolsMirror') }}</h3>
            <n-radio-group v-model:value="selected_tools_mirror" class="mirror-options"
              data-id="tools-mirror-radio-group">
              <div v-for="mirror in tools_mirrors" :key="mirror.value" class="mirror-option"
                :class="{ 'selected': selected_tools_mirror === mirror.value }"
                :data-id="`tools-mirror-option-${mirror.value}`"
                @click="selected_tools_mirror = mirror.value">
                <n-radio :value="mirror.value" :data-id="`tools-mirror-radio-${mirror.value}`">
                  <div class="mirror-content" :data-id="`tools-mirror-content-${mirror.value}`">
                    <span class="mirror-url" :data-id="`tools-mirror-url-${mirror.value}`">{{ mirror.label }}</span>
                    <span v-if="isDefaultMirror(mirror.value, 'tools')" class="mirror-tag"
                      :data-id="`tools-mirror-default-tag-${mirror.value}`">{{ t('mirrorSelect.tags.default') }}</span>
                  </div>
                </n-radio>
              </div>
            </n-radio-group>
          </div>

          <!-- PyPI Mirror Selection -->
          <div class="mirror-section" data-id="pypi-mirror-section">
            <h3 class="section-title" data-id="pypi-section-title">{{ t('mirrorSelect.sections.pypiMirror') }}</h3>
            <n-radio-group v-model:value="selected_pypi_mirror" class="mirror-options"
              data-id="pypi-mirror-radio-group">
              <div v-for="mirror in pypi_mirrors" :key="mirror.value" class="mirror-option"
                :class="{ 'selected': selected_pypi_mirror === mirror.value }"
                :data-id="`pypi-mirror-option-${mirror.value}`"
                @click="selected_pypi_mirror = mirror.value">
                <n-radio :value="mirror.value" :data-id="`pypi-mirror-radio-${mirror.value}`">
                  <div class="mirror-content" :data-id="`pypi-mirror-content-${mirror.value}`">
                    <span class="mirror-url" :data-id="`pypi-mirror-url-${mirror.value}`">{{ mirror.label }}</span>
                    <span v-if="isDefaultMirror(mirror.value, 'pypi')" class="mirror-tag"
                      :data-id="`pypi-mirror-default-tag-${mirror.value}`">{{ t('mirrorSelect.tags.default') }}</span>
                  </div>
                </n-radio>
              </div>
            </n-radio-group>
          </div>
        </div>

        <div class="action-footer" data-id="mirror-action-footer">
          <n-button @click="processChoices" type="error" size="large" :disabled="!canProceed"
            data-id="continue-mirrors-button">
            {{ t('mirrorSelect.continueButton') }}
          </n-button>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NRadio, NRadioGroup } from 'naive-ui'

import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'MirrorSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NRadio, NRadioGroup },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    loading_idfs: true,
    loading_tools: true,
    loading_pypi: true,
    selected_idf_mirror: null,
    selected_tools_mirror: null,
    selected_pypi_mirror: null,
    idf_mirrors: [],
    tools_mirrors: [],
    pypi_mirrors: [],
    defaultMirrors: {
      idf: '',
      tools: '',
      pypi: ''
    }
  }),
  methods: {
    get_available_idf_mirrors: async function () {
      const idf_mirrors = await invoke("get_idf_mirror_list", {});
      this.idf_mirrors = idf_mirrors.mirrors.map((mirror, index) => {
        return {
          value: mirror,
          label: mirror,
        }
      });
      this.selected_idf_mirror = idf_mirrors.selected;
      this.loading_idfs = false;
      return false;
    },
    get_available_tools_mirrors: async function () {
      const tools_mirrors = await invoke("get_tools_mirror_list", {});
      this.tools_mirrors = tools_mirrors.mirrors.map((mirror, index) => {
        return {
          value: mirror,
          label: mirror,
        }
      });
      this.selected_tools_mirror = tools_mirrors.selected;
      this.loading_tools = false;
      return false;
    },
    get_available_pypi_mirrors: async function () {
      const pypi_mirrors = await invoke("get_pypi_mirror_list", {});
      this.pypi_mirrors = pypi_mirrors.mirrors.map((mirror, index) => {
        return {
          value: mirror,
          label: mirror,
        }
      });
      this.selected_pypi_mirror = pypi_mirrors.selected;
      this.loading_pypi = false;
      return false;
    },
    isDefaultMirror(mirror, type) {
      return mirror === this.defaultMirrors[type];
    },
    processChoices: async function () {
      console.log("Mirror choices:", {
        idf_mirror: this.selected_idf_mirror,
        tools_mirror: this.selected_tools_mirror,
        pypi_mirror: this.selected_pypi_mirror
      });
      if (!this.loading_idfs && !this.loading_tools && !this.loading_pypi) {
        const _ = await invoke("set_idf_mirror", { mirror: this.selected_idf_mirror });
        const __ = await invoke("set_tools_mirror", { mirror: this.selected_tools_mirror });
        const ___ = await invoke("set_pypi_mirror", { mirror: this.selected_pypi_mirror });
        this.nextstep();
      }
    }
  },
  computed: {
    canProceed() {
      return this.selected_idf_mirror && this.selected_tools_mirror && this.selected_pypi_mirror &&
        !this.loading_idfs && !this.loading_tools && !this.loading_pypi;
    }
  },
  mounted() {
    this.get_available_idf_mirrors();
    this.get_available_tools_mirrors();
    this.get_available_pypi_mirrors();
  }
}
</script>

<style scoped>
.mirror-select {
  padding: 2rem;
  max-width: 1200px;
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

.mirrors-card {
  background: white;
  padding: 1.5rem;
}

.mirrors-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.5rem;
  margin-bottom: 2rem;
}

/* Responsive breakpoints */
@media (max-width: 1024px) {
  .mirrors-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 640px) {
  .mirrors-grid {
    grid-template-columns: 1fr;
  }
}

.mirror-section {
  padding: 0.4rem;
  background: #f9fafb;
  border-radius: 0.5rem;
  border: 1px solid #e5e7eb;
  display: flex;
  flex-direction: column;
  min-width: 0; /* Prevent overflow in grid */
}

.section-title {
  font-size: 1rem;
  font-weight: 600;
  color: #374151;
  margin-bottom: 1rem;
  padding-bottom: 0.75rem;
  border-bottom: 2px solid #e5e7eb;
}

.mirror-options {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  flex: 1;
}

.mirror-option {
  padding: 0.075rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.375rem;
  background: white;
  transition: all 0.2s ease;
  cursor: pointer;
}

.mirror-option:hover {
  border-color: #e7352c;
  box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
}

.mirror-option.selected {
  background-color: #fee2e2;
  border-color: #e7352c;
  box-shadow: 0 0 0 1px #e7352c;
}

.mirror-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  pointer-events: none;
}

.mirror-url {
  font-size: 0.875rem;
  color: #374151;
  word-break: break-all;
  flex: 1;
  min-width: 0;
}

.mirror-tag {
  font-size: 0.75rem;
  font-weight: 500;
  padding: 0.25rem 0.5rem;
  background-color: #e7352c;
  color: white;
  border-radius: 0.25rem;
  white-space: nowrap;
  flex-shrink: 0;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1.5rem;
  border-top: 1px solid #e5e7eb;
}

.n-card {
  border: none;
  padding: 0px;
}

.n-card__content {
  padding: 0px;
}

.n-radio .n-radio__dot.n-radio__dot--checked {
  display: none;
}

/* Ensure equal height for all mirror sections */
.mirrors-grid > .mirror-section {
  height: 100%;
}
</style>
