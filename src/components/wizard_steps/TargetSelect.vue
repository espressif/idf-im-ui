<template>
  <div class="target-select" data-id="target-select">
    <h1 class="title" data-id="target-select-title">Select Target Chips</h1>
    <p class="description" data-id="target-select-description">Choose the ESP chips you'll be developing for:</p>
    <p class="description" data-id="target-select-description-second-line">If you are not sure, you can consult our <a
        href="https://products.espressif.com/#/product-comparison?names=ESP32-S2,ESP32-C3,ESP32-S3,ESP32-C6&type=SoC"
        target="_blank">Product Selector</a></p>
    <div class="selection_header">
      Target Chips:
      <span @click="clickOnAll">
        All
        <n-checkbox :checked="all" id="select_all_targets" size="large"></n-checkbox>
      </span>
    </div>
    <n-card class="selection-card" data-id="target-selection-card">
      <n-spin :show="loading" data-id="target-loading-spinner">
        <div class="targets-grid" data-id="targets-grid">
          <div v-for="target in targets" :key="target.name" class="target-item"
            :class="{ 'selected': selected_targets.includes(target.name) }" :data-id="`target-item-${target.name}`"
            @click="clickOnTarget">

            <div class="target-content" :data-id="`target-content-${target.name}`">
              <span class="target-name" :data-id="`target-name-${target.name}`">{{ target.name }}</span>
              <span class="target-description" v-if="target.description" :data-id="`target-description-${target.name}`">
                {{ target.description }}
              </span>
            </div>

          </div>
        </div>

        <div class="action-footer" data-id="target-action-footer">
          <n-button @click="processTargets" type="error" size="large" :disabled="!hasSelectedTargets"
            data-id="continue-targets-button">
            Continue with Selected Targets
          </n-button>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NCheckbox } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'TargetSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NCheckbox },
  data: () => ({
    loading: true,
    targets: [],
    selected_targets: [],
    all: true,
  }),
  methods: {
    check_python_sanity: async function () {
      this.python_sane = await invoke("python_sanity_check", {});;

      return false;
    },
    get_available_targets: async function () {
      const targets = await invoke("get_available_targets", {});
      console.log('available targets', targets);
      this.targets = targets;
      this.loading = false;
      return false;
    },
    clickOnTarget: async function (event) {
      let chip_name = event.currentTarget.textContent.trim();
      console.log('target clicked', chip_name);
      if (this.selected_targets.includes(chip_name)) {
        this.selected_targets.splice(this.selected_targets.indexOf(chip_name), 1);
      } else {
        this.selected_targets.push(chip_name);
      }
      console.log('selected targets', this.selected_targets);
      if (this.selected_targets.length > 0) {
        this.all = false;
      } else {
        this.all = true;
      }
    },
    clickOnAll: async function (event) {
      console.log('all clicked', this.all);
      this.all = !this.all;
      this.selected_targets = this.all ? [] : this.selected_targets;
    },
    processTargets: async function () {
      const selected_targets = this.all ? ["all"] : this.selected_targets;
      const _ = await invoke("set_targets", { targets: selected_targets });
      this.nextstep();
    }
  },
  computed: {
    hasSelectedTargets() {
      return this.selected_targets.length > 0 || this.all;
    }
  },
  mounted() {
    this.get_available_targets();
  }
}
</script>

<style scoped>
.target-select {
  padding: 2rem;
  max-width: 800px;
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

hr {
  background-color: #6b7280;
  color: #6b7280;
}

.selection-card {
  background: white;
  padding: 0.001rem;
}

.targets-grid {
  display: flex;
  /* Use flexbox */
  flex-wrap: wrap;
  /* Allow items to wrap to the next line */
  gap: 13px;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  margin-bottom: 0.5rem;
}

.target-item {
  width: 125px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.5rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.5rem;
  cursor: pointer;
  transition: all 0.2s ease;
}

.target-item:hover {
  background-color: #f9fafb;
  border-color: #1290d8;
}

.target-item.selected {
  background-color: #1290d8;
  border-color: #1290d8;
  color: white
}

.target-content {
  display: flex;
  flex-direction: column;
  gap: 0rem;
}

.target-name {
  font-weight: 500;
  color: #374151;
}

.selected .target-name {
  color: white;
}

.target-description {
  font-size: 0.875rem;
  color: #6b7280;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
}

.n-card {
  border: none;
}

.selection_header {
  border-top: 1px solid #e5e7eb;
  padding-top: 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 21px;
}
</style>