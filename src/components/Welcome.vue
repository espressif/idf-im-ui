<template>
  <div class="welcome">
    <h1>Welcome to ESP-IDF Installation Manager!</h1>
    <p>This tool will guide you trough the installation process.</p>
    <n-button @click="getStarted" type="success">Get Started</n-button>
  </div>
</template>

<script>
import { NButton } from 'naive-ui'

export default {
  name: 'Welcome',
  components: { NButton },
  methods: {
    getStarted() {
      console.log('Getting started!');
      this.$router.push('/load_config');
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