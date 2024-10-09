<template>
  <p>Please select chips you want to develop for:</p>
  <n-space vertical>
    <n-spin :show="loading">
      <template #default>
        <ul>
          <li v-for="target in targets" :key="target">
            <span @click="clickOnTarget">
              <n-checkbox v-model:checked="target.selected">
                {{ target.name }}
              </n-checkbox>
            </span>
          </li>
        </ul>
      </template>
      <template #description>
        loading avalible targets...
      </template>
    </n-spin>

    <n-button @click="processTargets" type="primary">Next</n-button>
  </n-space>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'TargetSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    loading: true,
    targets: [],
  }),
  methods: {
    check_python_sanity: async function () {
      this.python_sane = await invoke("python_sanity_check", {});;

      return false;
    },
    get_avalible_targets: async function () {
      const targets = await invoke("get_available_targets", {});
      this.targets = targets.sort().map((target) => {
        return {
          name: target,
          // biome-ignore lint/suspicious/noDoubleEquals: <explanation>
          selected: target == 'all',
        }
      });
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
  mounted() {
    this.get_avalible_targets();
  }
}
</script>
