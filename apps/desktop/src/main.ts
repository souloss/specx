import { createApp } from "vue";
import { createPinia } from "pinia";
import { useTheme } from "./composables/useTheme";
import { i18n } from "./i18n";
import App from "./App.vue";
import "./styles/globals.css";

useTheme().applyTheme();
createApp(App).use(createPinia()).use(i18n).mount("#app");
