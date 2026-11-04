import "./assets/css/main.css";

import { createApp } from "vue";
import App from "./App.vue";
import ui from "@nuxt/ui/vue-plugin";
import { createRouter, createWebHashHistory } from "vue-router";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [],
});

createApp(App).use(router).use(ui).mount("#app");
