<template>
  <div class="installer">
    <!-- Header -->
    <header class="header">
      <h2>ESP-IDF Installation Manager</h2>
    </header>
    <router-view></router-view>
    <message-display />
    <global-progress />
    <!-- Footer -->
    <footer class="footer">
      ESP-IDF Installation Manager v1.0.0 (TODO:INSERT VERSION)
    </footer>
  </div>
</template>

<script setup>
import { NConfigProvider, NLayout, NLayoutHeader, NLayoutContent, useOsTheme } from 'naive-ui'
import { darkTheme } from 'naive-ui'
import { computed, onMounted } from 'vue'
import MessageDisplay from './components/MessageDisplay.vue'
import GlobalProgress from './components/GlobalProgress.vue'
import { attachConsole } from '@tauri-apps/plugin-log'
const osTheme = useOsTheme()
const theme = null // computed(() => (osTheme.value === 'dark' ? darkTheme : null))

onMounted(async () => {
  const detach = await attachConsole()
})
</script>