<template>
  <p>Please select download mirrors. If you are outside mainland china you probably want to use the defaults.</p>
  <n-split direction="horizontal" style="height: 200px" :max="0.75" :min="0.25">
    <template #1>
      <n-spin :show="loading_idfs">
        <template #default>
          <n-radio-group v-model:value="selected_idf_mirror" name="radiogroup">
            <ul>
              <n-space>
                <li v-for="mirror in idf_mirrors" :key="mirror.value">
                  <n-radio :value="mirror.value" :label="mirror.label" />
                </li>
              </n-space>
            </ul>
          </n-radio-group>
        </template>
        <template #description>
          loading available IDF download mirrors...
        </template>
      </n-spin>
    </template>
    <template #2>
      <n-spin :show="loading_tools">
        <template #default>
          <n-radio-group v-model:value="selected_tools_mirror" name="radiogroup">
            <ul>
              <n-space>
                <li v-for="mirror in tools_mirrors" :key="mirror.value">
                  <n-radio :value="mirror.value" :label="mirror.label" />
                </li>
              </n-space>
            </ul>
          </n-radio-group>
        </template>
        <template #description>
          loading available tools download mirrors...
        </template>
      </n-spin>
    </template>
  </n-split>

  <n-space>
    <n-button @click="processChoices" type="primary"
      :disabled="selected_idf_mirror == null || selected_tools_mirror == null">Next</n-button>
  </n-space>
</template>

<script>
import { ref, version } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'MirrorSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    loading_idfs: true,
    loading_tools: true,
    selected_idf_mirror: null,
    selected_tools_mirror: null,
    idf_mirrors: [],
    tools_mirrors: [],
  }),
  methods: {
    get_avalible_idf_mirrors: async function () {
      const idf_mirrors = await invoke("get_idf_mirror_list", {});
      this.idf_mirrors = idf_mirrors.map((mirror, index) => {
        return {
          value: mirror,
          label: mirror,
        }
      });
      this.selected_idf_mirror = this.idf_mirrors[0].value;
      this.loading_idfs = false;
      return false;
    },
    get_avalible_tools_mirrors: async function () {
      const tools_mirrors = await invoke("get_tools_mirror_list", {});
      this.tools_mirrors = tools_mirrors.map((mirror, index) => {
        return {
          value: mirror,
          label: mirror,
        }
      });
      this.selected_tools_mirror = this.tools_mirrors[0].value;
      this.loading_tools = false;
      return false;
    },
    processChoices: async function () {
      console.log("Mirror choices:", {
        idf_mirror: this.selected_idf_mirror,
        tools_mirror: this.selected_tools_mirror,
      });
      if (!this.loading_idfs && !this.loading_tools) {
        const _ = await invoke("set_idf_mirror", { mirror: this.selected_idf_mirror });
        const __ = await invoke("set_tools_mirror", { mirror: this.selected_tools_mirror });
        this.nextstep();
      }
    }
  },
  mounted() {
    this.get_avalible_idf_mirrors();
    this.get_avalible_tools_mirrors();
  }
}
</script>
