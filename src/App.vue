<template>
  <div class="installer">
    <!-- Header -->
    <header class="header">
      <img @click="goHome" class="main_logo" src="./assets/Espressif_White_Logo_EN_Horizontal.svg" alt="ESP-IDF Logo" />
      <h2>ESP-IDF Installation Manager</h2>
    </header>
    <router-view></router-view>
    <!-- Footer -->
    <footer class="footer">
      <div class="footer-content">
        <div class="version">ESP-IDF Installation Manager {{ appVersion }}</div>
        <div class="log-link">
          <LogLink />
        </div>
      </div>
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
import { useWizardStore } from './store'

export default {
  name: 'App',
  components: {
    NConfigProvider,
    NLayout,
    NLayoutHeader,
    NLayoutContent,
    LogLink
  },
  methods: {
    goHome() {
      let step = this.currentStep;
      if (step < 7) {
        this.$router.push('/')
      } else {
        console.log('Can not go back to home page')
      }
    }
  },
  computed: {
    store() {
      return useWizardStore()
    },
    currentStep() {
      return this.store.currentStep
    },
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

<style scoped>
.main_logo {
  cursor: pointer;
}
</style>