import { describe, expect, it } from "vitest";
import { renderResponseVisualization } from "./openapiResponseVisualization";
describe("response visualization templates", () => {
  it("renders repeated response fields while escaping untrusted values", () => {
    expect(renderResponseVisualization("<ul>{{#each items}}<li>{{name}}</li>{{/each}}</ul>", { items: [{ name: "<script>bad()</script>" }, { name: "fox" }] })).toBe("<ul><li>&lt;script&gt;bad()&lt;/script&gt;</li><li>fox</li></ul>");
  });
});
