<template>
  <p>Wizard will now check for the IDF Prerequisites...</p>
  <n-space vertical>
    <n-spin :show="loading">
      <n-alert title="List of needed Prerequisities" type="default">
        <ul>
          <li v-for="p in display_prerequisities" :key="p.name">{{ p.icon }} III {{ p.name }}</li>
        </ul>
      </n-alert>
      <template #description>
        Loading list of prerequisites...
      </template>
    </n-spin>

  </n-space>
  <n-button @click="check_prerequisites" type="primary">Check Prerequisites</n-button>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";


export default {
  name: 'PrerequisitiesCheck',
  components: { NButton, NSpin },
  data: () => ({
    loading: false,
    all_prerequisities: [],
    missing_prerequisities: [],
    display_prerequisities: [],
  }),
  methods: {
    get_prerequisities_list: async function () {
      this.loading = true;
      this.all_prerequisities = await invoke("get_prequisites", {});;
      this.loading = false;
      this.display_prerequisities = this.all_prerequisities.map(p => ({
        name: p,
        icon: '❓',
      }));
      return false;
    },
    check_prerequisites: async function () {
      this.loading = true;
      let pepa = await invoke("check_prequisites", {});
      this.missing_prerequisities = pepa;
      console.log("missing prerequisities: ", pepa);
      this.display_prerequisities = this.display_prerequisities.map(p => ({
        name: p.name,
        icon: pepa.includes(p.name) ? '❌' : '✔',
      }));
      this.loading = false;
      return false;
    }
  },
  mounted() {
    this.get_prerequisities_list();
  }
}
</script>

<!-- <script setup>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const loading = ref(false);
const loading_list = ref(false);
const all_prerequisities = ref([]);

async function checkprereq(e) {
  e.preventDefault();
  loading.value = true;
  // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
  greetMsg.value = await invoke("check_prereq", {});
  loading.value = false;
  return false;
} -->

<!-- </script> -->
