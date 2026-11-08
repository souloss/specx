import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiPreview from "./OpenApiPreview.vue";

const content = `openapi: 3.0.3
info:
  title: Accounts API
  version: '1.0'
servers:
  - url: https://api.example.test
paths:
  /accounts:
    get:
      operationId: listAccounts
      summary: List accounts
      parameters:
        - name: limit
          in: query
          required: false
          schema:
            type: integer
      responses:
        '200':
          description: Accounts returned
  /admin:
    delete:
      summary: Delete admin data
      responses:
        '204':
          description: Deleted
`;

describe("OpenAPI preview", () => {
  it("shows only the selected API definition and its own contract details", () => {
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts" } });
    expect(wrapper.text()).toContain("/accounts");
    expect(wrapper.text()).toContain("limit");
    expect(wrapper.text()).toContain("Accounts returned");
    expect(wrapper.text()).not.toContain("/admin");
    expect(wrapper.text()).not.toContain("api.example.test");
    expect(wrapper.text()).not.toContain("2 Operations");
  });

  it("parses YAML for the OpenAPI overview", () => {
    const wrapper = mount(OpenApiPreview, { props: { content } });
    expect(wrapper.text()).toContain("Accounts API");
    expect(wrapper.text()).toContain("api.example.test");
    expect(wrapper.text()).toContain("DELETE");
  });
});
