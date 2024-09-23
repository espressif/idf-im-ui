import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import naive from 'naive-ui'
import './assets/main.css' // Import the CSS file

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(naive)
app.mount('#app')