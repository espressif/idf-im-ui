<template>
  <div class="target-select">
    <h1 class="title">Select Target Chips</h1>
    <p class="description">Choose the ESP chips you'll be developing for:</p>

    <n-card class="selection-card">
      <n-spin :show="loading">
        <div class="targets-grid">
          <div v-for="target in targets" :key="target.name" class="target-item" :class="{ 'selected': target.selected }"
            @click="clickOnTarget">
            <n-checkbox v-model:checked="target.selected">
              <div class="target-content">
                <span class="target-name">{{ target.name }}</span>
                <span class="target-description" v-if="target.description">
                  {{ target.description }}
                </span>
              </div>
            </n-checkbox>
          </div>
        </div>

        <div class="action-footer">
          <n-button @click="processTargets" type="error" size="large" :disabled="!hasSelectedTargets">
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
  }),
  methods: {
    check_python_sanity: async function () {
      this.python_sane = await invoke("python_sanity_check", {});;

      return false;
    },
    get_available_targets: async function () {
      const targets = await invoke("get_available_targets", {});
      this.targets = targets;
      this.loading = false;
      return false;
    },
    clickOnTarget: async function (event) {
      if (event.currentTarget.textContent.toLowerCase().includes('all')) {
        console.log('all targets selected');
        if (this.targets[0].selected) {
          for (const t of this.targets) {
            t.selected = false;
          }
          this.targets[0].selected = true;
        }
      } else {
        this.targets[0].selected = false;
      }
    },
    processTargets: async function () {
      const selected_targets = this.targets.filter(target => target.selected).map(target => target.name);
      const _ = await invoke("set_targets", { targets: selected_targets });
      this.nextstep();
    }
  },
  computed: {
    hasSelectedTargets() {
      return this.targets.some(target => target.selected);
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
  font-size: 1.8rem;
  color: #374151;
  margin-bottom: 0.5rem;
}

.description {
  color: #6b7280;
  margin-bottom: 2rem;
}

.selection-card {
  background: white;
  padding: 0.001rem;
}

.targets-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 0.3rem;
  margin-bottom: 0.5rem;
}

.target-item {
  padding: 0.5rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.5rem;
  cursor: pointer;
  transition: all 0.2s ease;
}

.target-item:hover {
  background-color: #f9fafb;
  border-color: #e7352c;
}

.target-item.selected {
  background-color: #fee2e2;
  border-color: #e7352c;
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

.target-description {
  font-size: 0.875rem;
  color: #6b7280;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}
</style>