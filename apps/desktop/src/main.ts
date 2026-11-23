import { createApp } from "vue";
import { createPinia } from "pinia";
import { useTheme } from "./composables/useTheme";
import { i18n } from "./i18n";
import { publicOpenApiPublicationId } from "./lib/specifications/openapi/publishing/openapiPublication";

import "./styles/globals.css";

async function bootstrap() {
  const publicationId = publicOpenApiPublicationId();
  if (publicationId) {
    const { default: Reader } = await import("./components/specifications/openapi/publishing/OpenApiPublishedPage.vue");
    createApp(Reader, { id: publicationId }).use(i18n).mount("#app");
  } else {
    useTheme().applyTheme();
    const { default: App } = await import("./App.vue");
    createApp(App).use(createPinia()).use(i18n).mount("#app");
  }
}
void bootstrap();
