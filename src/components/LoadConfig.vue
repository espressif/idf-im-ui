<template>
  <div class="welcome">
    <h1>Please select starting point!</h1>
    <n-split direction="horizontal" style="height: 200px" :max="0.75" :min="0.25">
      <template #1>
        <div :style="{ height: '200px' }">
          ( TODO: put opemn config icon here )
          <n-button type="primary" ghost>
            Load instalation config.
          </n-button>
        </div>
      </template>
      <template #2>
        <div :style="{ height: '200px' }">

          ( TODO: put mighty wizard icon here )
          <n-button @click="startWizard" type="primary" ghost>
            Start Wizard.
          </n-button>
        </div>
      </template>
    </n-split>
  </div>
</template>

<script>
import { NSplit, NButton } from 'naive-ui'

export default {
  name: 'LoadConfig',
  components: { NSplit, NButton },
  methods: {
    startWizard() {
      this.$router.push('/wizard/1');
    }
  }
}
</script>

<style scoped>
.welcome {
  text-align: center;
  padding: 20px;
  margin-bottom: 30px;
  border: 1px 0px 1px 0px solid #ccc;
  border-radius: 10px;
  box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
}
</style>