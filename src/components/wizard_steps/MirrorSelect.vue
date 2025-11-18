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
            <n-radio-group v-model:value="selected_idf_mirror" class="mirror-options" data-id="idf-mirror-radio-group" @update:value="onSelectChange('idf')">
              <div v-for="mirror in idf_mirrors" :key="mirror.value" class="mirror-option"
                :class="{ 'selected': selected_idf_mirror === mirror.value }"
                :data-id="`idf-mirror-option-${mirror.value}`"
                @click="selected_idf_mirror = mirror.value">
                <n-radio :value="mirror.value" :data-id="`idf-mirror-radio-${mirror.value}`">
                  <div class="mirror-content" :data-id="`idf-mirror-content-${mirror.value}`">
                    <span class="mirror-url" :data-id="`idf-mirror-url-${mirror.value}`">{{ mirror.label }}</span>
                    <div class="mirror-subline" :data-id="`idf-mirror-subline-${mirror.value}`">
                      <template v-if="mirror.ping !== null">
                        <span v-if="mirror.ping > 0" class="mirror-ping" :data-id="`idf-mirror-ping-${mirror.value}`">
                          {{ mirror.ping + ' ms' }}
                        </span>
                        <span v-else class="status-badge timeout" :title="t('mirrorSelect.status.timeout')" :data-id="`idf-mirror-timeout-${mirror.value}`">
                          {{ t('mirrorSelect.status.timeout') }}
                        </span>
                      </template>
                    </div>
                  </div>
                </n-radio>
              </div>
            </n-radio-group>
          </div>

          <!-- Tools Mirror Selection -->
          <div class="mirror-section" data-id="tools-mirror-section">
            <h3 class="section-title" data-id="tools-section-title">{{ t('mirrorSelect.sections.toolsMirror') }}</h3>
            <n-radio-group v-model:value="selected_tools_mirror" class="mirror-options"
              data-id="tools-mirror-radio-group" @update:value="onSelectChange('tools')">
              <div v-for="mirror in tools_mirrors" :key="mirror.value" class="mirror-option"
                :class="{ 'selected': selected_tools_mirror === mirror.value }"
                :data-id="`tools-mirror-option-${mirror.value}`"
                @click="selected_tools_mirror = mirror.value">
                <n-radio :value="mirror.value" :data-id="`tools-mirror-radio-${mirror.value}`">
                  <div class="mirror-content" :data-id="`tools-mirror-content-${mirror.value}`">
                    <span class="mirror-url" :data-id="`tools-mirror-url-${mirror.value}`">{{ mirror.label }}</span>
                    <div class="mirror-subline" :data-id="`tools-mirror-subline-${mirror.value}`">
                      <template v-if="mirror.ping !== null">
                        <span v-if="mirror.ping > 0" class="mirror-ping" :data-id="`tools-mirror-ping-${mirror.value}`">
                          {{ mirror.ping + ' ms' }}
                        </span>
                        <span v-else class="status-badge timeout" :title="t('mirrorSelect.status.timeout')" :data-id="`tools-mirror-timeout-${mirror.value}`">
                          {{ t('mirrorSelect.status.timeout') }}
                        </span>
                      </template>
                    </div>
                  </div>
                </n-radio>
              </div>
            </n-radio-group>
          </div>

          <!-- PyPI Mirror Selection -->
          <div class="mirror-section" data-id="pypi-mirror-section">
            <h3 class="section-title" data-id="pypi-section-title">{{ t('mirrorSelect.sections.pypiMirror') }}</h3>
            <n-radio-group v-model:value="selected_pypi_mirror" class="mirror-options"
              data-id="pypi-mirror-radio-group" @update:value="onSelectChange('pypi')">
              <div v-for="mirror in pypi_mirrors" :key="mirror.value" class="mirror-option"
                :class="{ 'selected': selected_pypi_mirror === mirror.value }"
                :data-id="`pypi-mirror-option-${mirror.value}`"
                @click="selected_pypi_mirror = mirror.value">
                <n-radio :value="mirror.value" :data-id="`pypi-mirror-radio-${mirror.value}`">
                  <div class="mirror-content" :data-id="`pypi-mirror-content-${mirror.value}`">
                    <span class="mirror-url" :data-id="`pypi-mirror-url-${mirror.value}`">{{ mirror.label }}</span>
                    <div class="mirror-subline" :data-id="`pypi-mirror-subline-${mirror.value}`">
                      <template v-if="mirror.ping !== null">
                        <span v-if="mirror.ping > 0" class="mirror-ping" :data-id="`pypi-mirror-ping-${mirror.value}`">
                          {{ mirror.ping + ' ms' }}
                        </span>
                        <span v-else class="status-badge timeout" :title="t('mirrorSelect.status.timeout')" :data-id="`pypi-mirror-timeout-${mirror.value}`">
                          {{ t('mirrorSelect.status.timeout') }}
                        </span>
                      </template>
                    </div>
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
import { listen } from "@tauri-apps/api/event";
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
    },
    listeners: [],
    autoSelect: {
      idf: true,
      tools: true,
      pypi: true
    }
  }),
  methods: {
    sortMirrorsByPing(list) {
      list.sort((a, b) => {
        const ap = (a.ping !== null && a.ping > 0) ? a.ping : Number.POSITIVE_INFINITY;
        const bp = (b.ping !== null && b.ping > 0) ? b.ping : Number.POSITIVE_INFINITY;
        return ap - bp;
      });
    },
    onSelectChange(type) {
      // User has manually chosen a mirror for this type; stop auto-selecting
      if (this.autoSelect[type]) {
        this.autoSelect[type] = false;
      }
    },
    getBestMirror(list) {
      // best = smallest positive ping; ignore null (unknown) and 0 (timeout)
      const candidates = list.filter(m => m.ping !== null && m.ping > 0);
      if (candidates.length === 0) return null;
      let best = candidates[0];
      for (let i = 1; i < candidates.length; i++) {
        if (candidates[i].ping < best.ping) best = candidates[i];
      }
      return best;
    },
    maybeAutoSelectBest(type) {
      if (!this.autoSelect[type]) return;
      const list = type === 'idf' ? this.idf_mirrors : type === 'tools' ? this.tools_mirrors : this.pypi_mirrors;
      const best = this.getBestMirror(list);
      if (!best) return;
      const selectedKey = type === 'idf' ? 'selected_idf_mirror' : type === 'tools' ? 'selected_tools_mirror' : 'selected_pypi_mirror';
      const current = this[selectedKey];
      // If nothing selected or current is slower (or unknown), switch to best
      const currentEntry = list.find(m => m.value === current) || null;
      const currentPing = currentEntry ? currentEntry.ping : null;
      const currentScore = (currentPing !== null && currentPing > 0) ? currentPing : Number.POSITIVE_INFINITY;
      if (best.ping < currentScore) {
        this[selectedKey] = best.value;
      }
    },
    async get_available_idf_mirrors() {
      const res = await invoke("get_idf_mirror_urls", {});
      const urls = res.mirrors || [];
      const list = urls.map((url) => ({ value: url, label: url, ping: null }));
      this.idf_mirrors = list;
      this.selected_idf_mirror = res.selected || (list[0] ? list[0].value : null);
      this.defaultMirrors.idf = this.selected_idf_mirror || '';
      this.loading_idfs = false;
      // kick off streaming checks
      invoke("start_idf_mirror_latency_checks", {}).catch(() => {});
    },
    async get_available_tools_mirrors() {
      const res = await invoke("get_tools_mirror_urls", {});
      const urls = res.mirrors || [];
      const list = urls.map((url) => ({ value: url, label: url, ping: null }));
      this.tools_mirrors = list;
      this.selected_tools_mirror = res.selected || (list[0] ? list[0].value : null);
      this.defaultMirrors.tools = this.selected_tools_mirror || '';
      this.loading_tools = false;
      invoke("start_tools_mirror_latency_checks", {}).catch(() => {});
    },
    async get_available_pypi_mirrors() {
      const res = await invoke("get_pypi_mirror_urls", {});
      const urls = res.mirrors || [];
      const list = urls.map((url) => ({ value: url, label: url, ping: null }));
      this.pypi_mirrors = list;
      this.selected_pypi_mirror = res.selected || (list[0] ? list[0].value : null);
      this.defaultMirrors.pypi = this.selected_pypi_mirror || '';
      this.loading_pypi = false;
      invoke("start_pypi_mirror_latency_checks", {}).catch(() => {});
    },
    async setupLatencyListeners() {
      // IDF
      const un1 = await listen("idf-mirror-latency", (event) => {
        const { url, latency } = event.payload || {};
        const normalized = Number(latency) === 4294967295 ? 0 : (Number(latency) || 0);
        const idx = this.idf_mirrors.findIndex(m => m.value === url);
        if (idx !== -1) {
          this.idf_mirrors[idx].ping = normalized;
          this.sortMirrorsByPing(this.idf_mirrors);
          this.maybeAutoSelectBest('idf');
        }
      });
      const un1d = await listen("idf-mirror-latency-done", () => {});
      // Tools
      const un2 = await listen("tools-mirror-latency", (event) => {
        const { url, latency } = event.payload || {};
        const normalized = Number(latency) === 4294967295 ? 0 : (Number(latency) || 0);
        const idx = this.tools_mirrors.findIndex(m => m.value === url);
        if (idx !== -1) {
          this.tools_mirrors[idx].ping = normalized;
          this.sortMirrorsByPing(this.tools_mirrors);
          this.maybeAutoSelectBest('tools');
        }
      });
      const un2d = await listen("tools-mirror-latency-done", () => {});
      // PyPI
      const un3 = await listen("pypi-mirror-latency", (event) => {
        const { url, latency } = event.payload || {};
        const normalized = Number(latency) === 4294967295 ? 0 : (Number(latency) || 0);
        const idx = this.pypi_mirrors.findIndex(m => m.value === url);
        if (idx !== -1) {
          this.pypi_mirrors[idx].ping = normalized;
          this.sortMirrorsByPing(this.pypi_mirrors);
          this.maybeAutoSelectBest('pypi');
        }
      });
      const un3d = await listen("pypi-mirror-latency-done", () => {});
      this.listeners = [un1, un1d, un2, un2d, un3, un3d];
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
    this.setupLatencyListeners();
    this.get_available_idf_mirrors();
    this.get_available_tools_mirrors();
    this.get_available_pypi_mirrors();
  },
  beforeUnmount() {
    // cleanup listeners
    if (this.listeners && this.listeners.length) {
      this.listeners.forEach(un => {
        try { un(); } catch (_) {}
      });
      this.listeners = [];
    }
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
  flex-direction: column;
  align-items: flex-start;
  gap: 0.25rem;
  pointer-events: none;
}

.mirror-url {
  font-size: 0.875rem;
  color: #374151;
  overflow-wrap: anywhere;
  width: 100%;
}

.mirror-ping {
  font-size: 0.75rem;
  color: #6b7280;
  margin-right: 0;
}

.mirror-subline {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.status-badge {
  display: inline-flex;
  align-items: center;
  height: 20px;
  padding: 0 0.5rem;
  border-radius: 0.25rem;
  font-size: 0.75rem;
  font-weight: 500;
  line-height: 1;
  white-space: nowrap;
}

.status-badge.timeout {
  background-color: #f3f4f6; /* gray-100 */
  color: #6b7280;           /* gray-500 */
  border: 1px solid #e5e7eb;/* gray-200 */
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
