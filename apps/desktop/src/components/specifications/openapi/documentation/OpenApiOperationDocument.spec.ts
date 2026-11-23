import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiOperationDocument from "./OpenApiOperationDocument.vue";
import { getOpenApiOperations } from "@/lib/specifications/openapi/contract/openapi";

const document = {
  openapi: "3.1.0",
  paths: {
    "/pets": {
      post: {
        requestBody: { content: { "application/json": { examples: { cat: { value: { name: "Cat" } }, dog: { value: { name: "Dog" } } } }, "text/plain": { example: "Plain pet" } } },
        responses: {
          "200": { description: "Success", content: { "application/json": { schema: { type: "object", properties: { id: { type: "integer" } } }, example: { id: 1 } }, "text/plain": { schema: { type: "string" }, example: "Created pet" } } },
          "204": { description: "No content" },
        },
      },
    },
  },
};
function setup() {
  return mount(OpenApiOperationDocument, { props: { document, entry: getOpenApiOperations(JSON.stringify(document))[0]! } });
}

describe("operation document selections", () => {
  it("keeps code generation out while switching request body examples", async () => {
    const wrapper = setup();
    const body = wrapper.findAll(".openapi-media-document")[0]!;
    expect(wrapper.find('section[aria-label="请求代码示例"]').exists()).toBe(false);
    await body.get('button[aria-label="选择示例"] + select.app-select-native').setValue("dog");
    expect(body.get(".openapi-code-sample").text()).toContain("Dog");
    await body.get('button[aria-label="媒体类型"] + select.app-select-native').setValue("text/plain");
    expect(body.get(".openapi-code-sample").text()).toBe("Plain pet");
    expect(wrapper.find('section[aria-label="请求代码示例"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it("renders the documentation examples once and keeps the response example status in sync", async () => {
    const wrapper = mount(OpenApiOperationDocument, { props: { document, entry: getOpenApiOperations(JSON.stringify(document))[0]!, showExamples: true } });
    const examples = wrapper.get(".openapi-document-examples");
    expect(examples.findAll("h4").filter((heading) => heading.text() === "请求代码示例")).toHaveLength(1);
    expect(examples.text()).toContain("请求代码示例");
    const noContent = examples.findAll('[role="tab"]').find((tab) => tab.text().startsWith("204"));
    expect(noContent).toBeDefined();
    await noContent!.trigger("click");
    expect(examples.get('[role="tab"][aria-selected="true"]').text()).toContain("204");
    expect(examples.text()).toContain("此响应未定义正文。");
    wrapper.unmount();
  });

  it("keeps response schema, status and example selections in one panel", async () => {
    const wrapper = setup();
    const response = wrapper.get(".openapi-response-document");
    await response.get('button[aria-label="媒体类型"] + select.app-select-native').setValue("text/plain");
    expect(response.get<HTMLSelectElement>('button[aria-label="媒体类型"] + select.app-select-native').element.value).toBe("text/plain");
    expect(response.text()).toContain("Created pet");
    const noContent = response.findAll('[role="tab"]').find((button) => button.text().startsWith("204"))!;
    await noContent.trigger("click");
    expect(response.get('button[aria-selected="true"]').text()).toContain("204");
    expect(response.text()).toContain("此响应未定义正文。");
    wrapper.unmount();
  });
});
