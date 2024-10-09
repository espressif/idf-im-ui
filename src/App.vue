<template>
  <n-config-provider :theme="theme">
    <n-layout>
      <n-layout-header>ESP-IDF Installation Manager(EIM)</n-layout-header>
      <n-layout-content>
        <router-view></router-view>
      </n-layout-content>
      <message-display />
    </n-layout>
  </n-config-provider>
</template>

<script setup>
import { NConfigProvider, NLayout, NLayoutHeader, NLayoutContent, useOsTheme } from 'naive-ui'
import { darkTheme } from 'naive-ui'
import { computed, onMounted } from 'vue'
import MessageDisplay from './components/MessageDisplay.vue'
import { attachConsole } from '@tauri-apps/plugin-log'
const osTheme = useOsTheme()
const theme = null // computed(() => (osTheme.value === 'dark' ? darkTheme : null))

onMounted(async () => {
  const detach = await attachConsole()
})
</script>