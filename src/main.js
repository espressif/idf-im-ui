import { createApp } from "vue";
import { createI18n } from "vue-i18n";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import naive from "naive-ui";
import "./assets/main.css"; // Import the CSS file

// Translation files
import en from "./locales/en.json";
import cn from "./locales/cn.json";

const i18n = createI18n({
  legacy: false,
  locale: "en", // default locale
  fallbackLocale: "en",
  messages: {
    en,
    cn,
  },
});

const app = createApp(App);
app.use(i18n);
app.use(createPinia());
app.use(router);
app.use(naive);
app.mount("#app");
