<template>
  <n-config-provider :theme="theme">
    <n-layout>
      <n-layout-header>ESP-IDF Installation Manager(EIM)</n-layout-header>
      <n-layout-content>
        <router-view></router-view>
      </n-layout-content>
    </n-layout>
  </n-config-provider>
</template>

<script>
import { NConfigProvider, NLayout, NLayoutHeader, NLayoutContent, useOsTheme } from 'naive-ui'
import { darkTheme } from 'naive-ui'
import { computed } from 'vue'

export default {
  components: {
    NConfigProvider,
    NLayout,
    NLayoutHeader,
    NLayoutContent
  },
  setup() {
    const osTheme = useOsTheme()
    const theme = null // computed(() => (osTheme.value === 'dark' ? darkTheme : null))

    return {
      theme
    }
  }
}
</script>