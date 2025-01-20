<template>
  <div class="installer">
    <!-- Header -->
    <header class="header">
      <img src="./assets/Espressif_White_Logo_EN_Horizontal.svg" alt="ESP-IDF Logo" />
      <h2>ESP-IDF Installation Manager</h2>
    </header>
    <router-view></router-view>
    <!-- Footer -->
    <footer class="footer">
      ESP-IDF Installation Manager {{ appVersion }}
    </footer>
  </div>
</template>

<script setup>
import { NConfigProvider, NLayout, NLayoutHeader, NLayoutContent, useOsTheme } from 'naive-ui'
import { darkTheme } from 'naive-ui'
import { ref, onMounted } from 'vue'
import { attachConsole } from '@tauri-apps/plugin-log'
import { getVersion } from '@tauri-apps/api/app';


const osTheme = useOsTheme()
const theme = null // computed(() => (osTheme.value === 'dark' ? darkTheme : null))
const appVersion = ref('');

onMounted(async () => {
  const detach = await attachConsole();
  appVersion.value = await getVersion();
})
</script>