<template>
  <div class="simple-setup">
    <div class="setup-header">
      <h1 class="title">{{ $t('configSetup.title') }}</h1>
      <n-button @click="goBack" quaternary text-color="white">
        <template #icon>
          <n-icon><ArrowLeftOutlined /></n-icon>
        </template>
        {{ $t('configSetup.back') }}
      </n-button>
    </div>

    <!-- Installation Progress -->
    <n-card class="status-card" bordered>
      <InstalationProgress data-id="installation-progress" />
    </n-card>
  </div>
</template>

<script>
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  NButton, NCard, NIcon
} from 'naive-ui'
import {
  ArrowLeftOutlined,
} from '@vicons/antd'
import InstalationProgress from './wizard_steps/InstalationProgress.vue';


export default {
  name: 'SimpleInstallatioProgressWrapper',
  components: {
    NButton, NCard, NIcon,
    ArrowLeftOutlined, InstalationProgress
  },
  setup() {
    const router = useRouter()
    const { t } = useI18n()

    const goBack = () => {
      router.push('/basic-installer')
    }

    return {
      t,
      goBack,
      InstalationProgress
    }
  }
}
</script>

<style scoped>
.n-card {
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  border-radius: 8px;
  text-align: center;
  justify-content: center;
}
.simple-setup {
  padding: 2rem;
  /* max-width: 900px; */
  margin: 0 auto;
}

.setup-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
}

.title {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 2rem;
  color: #1f2937;
  margin: 0;
}

.status-card {
  background: white;
  padding: 2rem;
}
.n-button {
  font-family: 'Trueno-regular', sans-serif;
  font-size: 1rem;
  color: white;
}
</style>
