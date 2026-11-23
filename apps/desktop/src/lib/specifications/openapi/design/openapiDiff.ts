import { getOpenApiOperations, isRecord, parseOpenApiContent, resolveOpenApiRef } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";

export type OpenApiDiffSeverity = "breaking" | "safe";
export type OpenApiDiffChange = {
  severity: OpenApiDiffSeverity;
  kind: "operation" | "parameter" | "requestBody" | "response" | "schema";
  path: string;
  message: string;
};

export type OpenApiDiffResult = {
  changes: OpenApiDiffChange[];
  breakingCount: number;
  safeCount: number;
  impact: "major" | "minor" | "patch";
};

function operationMap(document: OpenApiObject | null): Map<string, ReturnType<typeof getOpenApiOperations>[number]> {
  if (!document) return new Map();
  const operations = getOpenApiOperations(JSON.stringify(document));
  return new Map(operations.map((operation) => [operation.identityKey, operation]));
}

function responseStatuses(operation: OpenApiObject): Set<string> {
  return new Set(isRecord(operation.responses) ? Object.keys(operation.responses) : []);
}

function requiredParameters(operation: OpenApiObject): Set<string> {
  if (!Array.isArray(operation.parameters)) return new Set();
  return new Set(
    operation.parameters
      .filter(isRecord)
      .filter((parameter) => parameter.required === true)
      .map((parameter) => `${String(parameter.in)}:${String(parameter.name)}`),
  );
}

function parameterMap(operation: OpenApiObject): Map<string, Record<string, unknown>> {
  if (!Array.isArray(operation.parameters)) return new Map();
  return new Map(operation.parameters.filter(isRecord).map((parameter) => [`${String(parameter.in)}:${String(parameter.name)}`, parameter]));
}

function schemaProperties(document: OpenApiObject | null, value: unknown): { required: Set<string>; properties: Set<string>; type: string | null } {
  const schema = resolveOpenApiRef(document, value);
  if (!schema) return { required: new Set(), properties: new Set(), type: null };
  const properties = isRecord(schema.properties) ? new Set(Object.keys(schema.properties)) : new Set<string>();
  const required = new Set(Array.isArray(schema.required) ? schema.required.filter((key): key is string => typeof key === "string") : []);
  const type = typeof schema.type === "string" ? schema.type : schema.properties ? "object" : null;
  return { required, properties, type };
}

function responseSchema(operation: OpenApiObject, status: string): unknown {
  const response = isRecord(operation.responses) ? operation.responses[status] : null;
  if (!isRecord(response) || !isRecord(response.content)) return null;
  const media = isRecord(response.content["application/json"]) ? response.content["application/json"] : Object.values(response.content).find(isRecord);
  return isRecord(media) ? media.schema : null;
}

export function compareOpenApiDocuments(beforeContent: string, afterContent: string): OpenApiDiffResult {
  const before = parseOpenApiContent(beforeContent);
  const after = parseOpenApiContent(afterContent);
  const changes: OpenApiDiffChange[] = [];
  const beforeOperations = operationMap(before);
  const afterOperations = operationMap(after);
  for (const [identity, operation] of beforeOperations) {
    if (!afterOperations.has(identity)) changes.push({ severity: "breaking", kind: "operation", path: identity, message: "operation_removed" });
    else {
      const next = afterOperations.get(identity)!;
      const beforeRequired = requiredParameters(operation.operation);
      const afterRequired = requiredParameters(next.operation);
      for (const parameter of afterRequired) if (!beforeRequired.has(parameter)) changes.push({ severity: "breaking", kind: "parameter", path: `${identity} ${parameter}`, message: "required_parameter_added" });
      for (const parameter of beforeRequired) if (!afterRequired.has(parameter)) changes.push({ severity: "safe", kind: "parameter", path: `${identity} ${parameter}`, message: "required_parameter_removed" });
      const beforeParameters = parameterMap(operation.operation);
      const afterParameters = parameterMap(next.operation);
      for (const [parameter, beforeValue] of beforeParameters) {
        const afterValue = afterParameters.get(parameter);
        if (!afterValue) continue;
        const beforeType = schemaProperties(before, beforeValue.schema).type;
        const afterType = schemaProperties(after, afterValue.schema).type;
        if (beforeType && afterType && beforeType !== afterType) changes.push({ severity: "breaking", kind: "parameter", path: `${identity} ${parameter}`, message: "parameter_type_changed" });
      }
      const beforeBody = isRecord(operation.operation.requestBody) ? operation.operation.requestBody : null;
      const afterBody = isRecord(next.operation.requestBody) ? next.operation.requestBody : null;
      if (!beforeBody && afterBody && afterBody.required === true) changes.push({ severity: "breaking", kind: "requestBody", path: identity, message: "required_request_body_added" });
      if (beforeBody && afterBody) {
        if (beforeBody.required !== true && afterBody.required === true) changes.push({ severity: "breaking", kind: "requestBody", path: identity, message: "request_body_now_required" });
        const beforeBodySchema = isRecord(beforeBody.content) ? Object.values(beforeBody.content).find(isRecord) : null;
        const afterBodySchema = isRecord(afterBody.content) ? Object.values(afterBody.content).find(isRecord) : null;
        const beforeBodyType = schemaProperties(before, isRecord(beforeBodySchema) ? beforeBodySchema.schema : null).type;
        const afterBodyType = schemaProperties(after, isRecord(afterBodySchema) ? afterBodySchema.schema : null).type;
        if (beforeBodyType && afterBodyType && beforeBodyType !== afterBodyType) changes.push({ severity: "breaking", kind: "requestBody", path: identity, message: "request_body_type_changed" });
      }
      for (const status of responseStatuses(operation.operation)) if (!responseStatuses(next.operation).has(status)) changes.push({ severity: "breaking", kind: "response", path: `${identity} ${status}`, message: "response_removed" });
      for (const status of responseStatuses(next.operation)) if (!responseStatuses(operation.operation).has(status)) changes.push({ severity: "safe", kind: "response", path: `${identity} ${status}`, message: "response_added" });
      for (const status of responseStatuses(operation.operation)) {
        if (!responseStatuses(next.operation).has(status)) continue;
        const beforeSchema = schemaProperties(before, responseSchema(operation.operation, status));
        const afterSchema = schemaProperties(after, responseSchema(next.operation, status));
        if (beforeSchema.type && afterSchema.type && beforeSchema.type !== afterSchema.type) changes.push({ severity: "breaking", kind: "schema", path: `${identity} ${status}`, message: "response_type_changed" });
        for (const property of beforeSchema.required) if (!afterSchema.properties.has(property)) changes.push({ severity: "breaking", kind: "schema", path: `${identity} ${status}.${property}`, message: "required_response_property_removed" });
        for (const property of afterSchema.properties)
          if (!beforeSchema.properties.has(property))
            changes.push({ severity: afterSchema.required.has(property) ? "breaking" : "safe", kind: "schema", path: `${identity} ${status}.${property}`, message: afterSchema.required.has(property) ? "required_response_property_added" : "response_property_added" });
      }
    }
  }
  for (const identity of afterOperations.keys()) if (!beforeOperations.has(identity)) changes.push({ severity: "safe", kind: "operation", path: identity, message: "operation_added" });
  const beforeSchemas = isRecord(before?.components) && isRecord(before.components.schemas) ? before.components.schemas : {};
  const afterSchemas = isRecord(after?.components) && isRecord(after.components.schemas) ? after.components.schemas : {};
  for (const name of Object.keys(beforeSchemas)) {
    if (!(name in afterSchemas)) changes.push({ severity: "breaking", kind: "schema", path: `components.schemas.${name}`, message: "schema_removed" });
    else {
      const beforeType = schemaProperties(before, beforeSchemas[name]).type;
      const afterType = schemaProperties(after, afterSchemas[name]).type;
      if (beforeType && afterType && beforeType !== afterType) changes.push({ severity: "breaking", kind: "schema", path: `components.schemas.${name}`, message: "schema_type_changed" });
    }
  }
  for (const name of Object.keys(afterSchemas)) if (!(name in beforeSchemas)) changes.push({ severity: "safe", kind: "schema", path: `components.schemas.${name}`, message: "schema_added" });
  const breakingCount = changes.filter((change) => change.severity === "breaking").length;
  const safeCount = changes.length - breakingCount;
  return { changes, breakingCount, safeCount, impact: breakingCount ? "major" : safeCount ? "minor" : "patch" };
}
