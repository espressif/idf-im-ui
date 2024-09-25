<template>
  <p>Wizard will now check your Python instalation...</p>
  <n-space vertical>
    <n-spin :show="loading">
      <template #default>
        <ul>
          <li v-for="target in targets" :key="target">
            <n-checkbox v-model:checked="target.selected" @click="clickOnTarget">
              {{ target.name }}
            </n-checkbox>
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
      let targets = await invoke("get_available_targets", {});
      this.targets = targets.sort().map((target) => {
        return {
          name: target,
          selected: target == 'all' ? true : false,
        }
      });
      this.loading = false;
      return false;
    },
    clickOnTarget: function (target) {
      if (target.target.innerText == 'all') {
        if (this.targets[0].selected) {
          this.targets.forEach(t => t.selected = false);
          this.targets[0].selected = true;
        }
      } else {
        this.targets[0].selected = false;
      }
    },
    processTargets: function () {
      let selected_targets = this.targets.filter(target => target.selected);
      // selected_targets TODO: send to backend
      this.nextstep();
    }
  },
  mounted() {
    this.get_avalible_targets();
  }
}
</script>
