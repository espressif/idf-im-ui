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
      <LogLink />
    </footer>
  </div>
</template>

<script>
import { NConfigProvider, NLayout, NLayoutHeader, NLayoutContent, useOsTheme } from 'naive-ui'
import { darkTheme } from 'naive-ui'
import { ref, onMounted } from 'vue'
import { attachConsole } from '@tauri-apps/plugin-log'
import { getVersion } from '@tauri-apps/api/app'
import LogLink from './components/LogLink.vue'

export default {
  name: 'App',
  components: {
    NConfigProvider,
    NLayout,
    NLayoutHeader,
    NLayoutContent,
    LogLink
  },
  setup() {
    const osTheme = useOsTheme()
    const theme = null // If you want to use computed: const theme = computed(() => (osTheme.value === 'dark' ? darkTheme : null))
    const appVersion = ref('')

    onMounted(async () => {
      const detach = await attachConsole()
      appVersion.value = await getVersion()
    })

    return {
      osTheme,
      theme,
      appVersion
    }
  }
}
</script>