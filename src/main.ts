import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";

import "@fortawesome/fontawesome-free/css/all.min.css";
import "./style.css";

createApp(App).use(createPinia()).mount("#app");
