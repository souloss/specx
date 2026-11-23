import { validateOpenApiSchema, type OpenApiSchemaValidationInput } from "./openapiSchemaValidation";

self.addEventListener("message", (event: MessageEvent<OpenApiSchemaValidationInput>) => {
  self.postMessage(validateOpenApiSchema(event.data));
});
