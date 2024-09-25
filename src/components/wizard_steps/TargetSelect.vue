<template>
  <p>Wizard will now check your Python instalation...</p>
  <n-space vertical>
    <n-spin :show="loading">
      <template #default>
        <ul>
          <li v-for="target in targets" :key="target">{{ target.name }}</li>
        </ul>
      </template>
      <template #description>
        loading avalible targets...
      </template>
    </n-spin>

    <n-button @click="nextstep" type="primary">Next</n-button>
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
      this.targets = targets.map((target) => {
        return {
          name: target,
          selected: target == 'all' ? true : false,
        }
      });
      this.loading = false;
      return false;
    },
  },
  mounted() {
    this.get_avalible_targets();
  }
}
</script>
