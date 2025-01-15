<template>


  <!-- Main content -->
  <main class="main-content">
    <div class="welcome-card">
      <h1>Welcome to <span>ESP-IDF</span> Installation Manager!</h1>

      <div class="content">
        <p>This tool will guide you through the installation process.</p>

        <div class="features">
          <div class="feature">
            <div class="feature-icon">✓</div>
            <span>Install required dependencies</span>
          </div>

          <div class="feature">
            <div class="feature-icon">✓</div>
            <span>Configure development environment</span>
          </div>

          <div class="feature">
            <div class="feature-icon">✓</div>
            <span>Set up ESP-IDF toolchain</span>
          </div>
        </div>
      </div>

      <n-button @click="getStarted" type="error" size="large" class="get-started">
        Get Started
      </n-button>
    </div>
  </main>


</template>

<script>
import { NButton } from 'naive-ui'

export default {
  name: 'Welcome',
  components: { NButton },
  methods: {
    getStarted() {
      this.$router.push('/load_config');
    }
  }
}
</script>



<style scoped>
.installer {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: #f5f5f5;
}

.header {
  background-color: #dc2626;
  color: white;
  padding: 1rem;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.header h2 {
  margin: 0;
  font-size: 1.25rem;
}


.welcome-card {
  background: white;
  padding: 2rem;
  border-radius: 0.5rem;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  max-width: 948px;
  width: 100%;
  text-align: center;
}

.welcome-card h1 {
  font-family: 'Trueno-bold', sans-serif;
  font-weight: bold;
  font-size: 2rem;
  color: #1f2937;
  margin-bottom: 1.5rem;
}

.welcome-card h1 span {
  color: #E8362D
}

.content {
  margin-bottom: 2rem;
}

.content p {
  color: #4b5563;
  font-family: 'Trueno-regular', sans-serif;
  font-size: 1.5rem;
  font-weight: bold;
  margin-bottom: 1.5rem;
}

.features {
  display: flex;
  flex-direction: row;
  justify-content: space-between;
  align-items: center;
  max-width: 800px;
  margin: 0 auto;
  padding: 20px;
  position: relative;
}

.feature {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  position: relative;
  width: 250px;
  height: 65px;
}

.feature span {

  font-size: 1.3rem;
  color: #4b5563;
}

.feature-icon {
  background-color: #1290d8;
  font-size: 1.6rem;
  color: white;
  width: 2rem;
  height: 2rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 10px;
  z-index: 1;
}

.feature:not(:last-child)::after {
  content: '';
  position: absolute;
  top: 15px;
  left: 65%;
  width: calc(100% - 65px);
  height: 4px;
  border-radius: 2px;
  background-color: #2196F3;
}

.get-started {
  width: 100%;
  max-width: 175px;
  margin: 0 auto;
  margin-top: 1.5rem;
  background-color: #E8362D;
}

.footer {
  padding: 1rem;
  text-align: center;
  color: #6b7280;
  font-size: 0.875rem;
}
</style>