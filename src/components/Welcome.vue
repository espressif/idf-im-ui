<template>


  <!-- Main content -->
  <main class="main-content">
    <div class="welcome-card">
      <h1>Welcome to ESP-IDF Installation Manager!</h1>

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
  max-width: 600px;
  width: 100%;
}

.welcome-card h1 {
  font-size: 1.875rem;
  color: #1f2937;
  margin-bottom: 1.5rem;
}

.content {
  margin-bottom: 2rem;
}

.content p {
  color: #4b5563;
  font-size: 1.125rem;
  margin-bottom: 1.5rem;
}

.features {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.feature {
  display: flex;
  align-items: center;
  color: #4b5563;
}

.feature-icon {
  background-color: #fee2e2;
  color: #dc2626;
  width: 2rem;
  height: 2rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 0.75rem;
}

.get-started {
  width: 100%;
  margin-top: 1.5rem;
}

.footer {
  padding: 1rem;
  text-align: center;
  color: #6b7280;
  font-size: 0.875rem;
}
</style>