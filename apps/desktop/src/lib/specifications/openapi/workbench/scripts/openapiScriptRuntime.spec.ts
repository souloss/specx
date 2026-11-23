// @vitest-environment node
import { beforeAll, describe, expect, it } from "vitest";
import { newQuickJSWASMModuleFromVariant } from "quickjs-emscripten-core";
import type { QuickJSWASMModule } from "quickjs-emscripten-core";
import RELEASE_SYNC from "@jitl/quickjs-wasmfile-release-sync";
import { executeOpenApiScript } from "./openapiScriptRuntime";
import type { OpenApiScriptInput } from "./openapiScriptSandbox";
import { RESPONSE_VISUALIZATION_DEMO } from "../response/openapiResponseVisualization";

let vm: QuickJSWASMModule;
beforeAll(async () => {
  vm = await newQuickJSWASMModuleFromVariant(RELEASE_SYNC);
});
function input(script: string, post = false): OpenApiScriptInput {
  return {
    kind: post ? "postResponse" : "preRequest",
    script,
    variables: { baseUrl: "https://api.test" },
    request: { method: "POST", url: "https://api.test/posts?old=1", headers: [{ name: "X-Example", value: "old", enabled: false }], cookies: [], body: '{"draft":true}', mediaType: "application/json" },
    ...(post ? { response: { status: 200, statusText: "OK", duration: 12, headers: [["Content-Type", "application/json"]], body: '{"id":1,"title":"hello"}', truncated: false } } : {}),
  };
}

describe("real QuickJS script runtime", () => {
  it("resolves dynamic expressions and nested variables in actual pre/post VMs", async () => {
    const source = input(`const name=await pm.variables.replaceInAsync('{{$person.fullName(locale="en")}}');pm.request.setHeader('X-Generated',name);pm.variables.set('number','{{$number.int(min=42,max=42)}}');pm.test('nested',()=>pm.expect(pm.variables.replaceIn('{{number}}')).to.equal('42'));`);
    const result = await executeOpenApiScript(vm, source);
    expect(result.request.headers.find((row) => row.name === "X-Generated")?.value).toMatch(/[A-Za-z]/);
    expect(result.tests[0]?.passed).toBe(true);
  });
  it("enables and updates an existing request header without changing the editor draft", async () => {
    const source = input('specx.request.setHeader("X-Example", "value");');
    const result = await executeOpenApiScript(vm, source);
    expect(result.request.headers).toEqual([{ name: "X-Example", value: "value", enabled: true }]);
    expect(source.request.headers[0]?.value).toBe("old");
  });
  it("supports console logging of structured values", async () => {
    const result = await executeOpenApiScript(vm, input('console.log("payload", {id:1}); console.warn("warning");'));
    expect(result.logs).toEqual(['payload {"id":1}', "[warn] warning"]);
  });
  it("renders the user's exact nested Handlebars example", async () => {
    const result = await executeOpenApiScript(vm, input(RESPONSE_VISUALIZATION_DEMO, true));
    expect(result.visualization).toContain("<dt>id</dt><dd>1</dd>");
    expect(result.visualization).toContain("<dt>title</dt><dd>hello</dd>");
  });
  it("executes pm request headers, URL, raw body and variable APIs", async () => {
    const result = await executeOpenApiScript(
      vm,
      input(`
      pm.variables.set("name", "new");
      pm.request.headers.upsert({key:"x-example",value:pm.variables.replaceIn("{{name}}")});
      pm.request.headers.add({key:"X-Extra",value:"ok"});
      pm.request.url.query.upsert({key:"old",value:"2"});
      pm.request.body.update('{"sent":true}');
      pm.request.method="PUT";
    `),
    );
    expect(result.request.method).toBe("PUT");
    expect(result.request.url).toBe("https://api.test/posts?old=2");
    expect(result.request.body).toBe('{"sent":true}');
    expect(result.request.headers).toContainEqual({ name: "x-example", value: "new", enabled: true });
    expect(result.variables.name).toBe("new");
  });
  it("records passing and failing Chai assertions without losing the response or later tests", async () => {
    const result = await executeOpenApiScript(
      vm,
      input(
        `
      pm.test("status", () => pm.response.to.have.status(200));
      pm.test("payload", () => pm.expect(pm.response.json()).to.deep.include({id:1}));
      pm.test("intentional failure", () => pm.expect(pm.response.code).to.equal(201));
      pm.test("continues", () => pm.expect(pm.response.headers.get("content-type")).to.include("json"));
    `,
        true,
      ),
    );
    expect(result.tests.map((test) => test.passed)).toEqual([true, true, false, true]);
    expect(result.tests[2]?.error).toContain("201");
  });
  it("waits for promises and timers before returning mutations", async () => {
    const result = await executeOpenApiScript(vm, input('await new Promise(resolve => setTimeout(resolve, 10)); pm.request.setHeader("X-Async", "done");'));
    expect(result.request.headers).toContainEqual({ name: "X-Async", value: "done", enabled: true });
  });
  it("has no browser, host or filesystem access", async () => {
    const result = await executeOpenApiScript(vm, input("specx.log(typeof fetch, typeof window, typeof process, typeof require);"));
    expect(result.logs).toEqual(["undefined undefined undefined undefined"]);
  });
  it("preserves logs and assertions before a runtime error", async () => {
    await expect(executeOpenApiScript(vm, input('console.log("before"); pm.test("ok",()=>pm.expect(1).to.equal(1)); throw new Error("broken");'))).rejects.toMatchObject({ message: "broken", output: { logs: ["before"], tests: [{ name: "ok", passed: true }] } });
  });
  it("handles non-JSON responses and only parses JSON on demand", async () => {
    const source = input('pm.test("text",()=>pm.expect(pm.response.text()).to.equal("hello"));', true);
    source.response!.body = "hello";
    expect((await executeOpenApiScript(vm, source)).tests[0]?.passed).toBe(true);
    await expect(executeOpenApiScript(vm, { ...source, script: "pm.response.json();" })).rejects.toThrow();
  });
  it("explains use of response APIs in a pre-request script", async () => {
    await expect(executeOpenApiScript(vm, input("pm.response.json();"))).rejects.toThrow("only available in post-response");
  });
  it("explains why post-response request header changes cannot affect the sent request", async () => {
    const source = input('specx.request.setHeader("X-Example", "too late"); pm.request.method="PUT";', true);
    source.messages = { requestAlreadySent: "请求已发送，请在前置脚本中修改。" };
    const result = await executeOpenApiScript(vm, source);
    expect(result.logs).toEqual(["[warn] 请求已发送，请在前置脚本中修改。"]);
    expect(result.response?.headers).not.toContainEqual(["X-Example", "too late"]);
  });
  it("keeps environment/global/local scopes distinct and returns their mutations", async () => {
    const source = input(`
      pm.environment.set("name","environment");pm.globals.set("name","global");
      pm.test("environment takes priority",()=>pm.expect(pm.variables.get("name")).to.equal("environment"));
      pm.variables.set("name","temporary");
      pm.test("temporary takes priority",()=>pm.expect(pm.variables.get("name")).to.equal("temporary"));
      pm.environment.unset("old");
    `);
    source.scopes = { environment: { old: "remove" }, globals: {} };
    const result = await executeOpenApiScript(vm, source);
    expect(result.scopes).toEqual({ environment: { name: "environment" }, globals: { name: "global" } });
    expect(result.variables.name).toBe("temporary");
    expect(result.tests.every((test) => test.passed)).toBe(true);
  });
  it("preserves temporary overrides across independently executed script steps", async () => {
    const source = { ...input('pm.variables.set("name", "temporary");'), variables: { name: "environment" }, scopes: { environment: { name: "environment" }, globals: {} } };
    const first = await executeOpenApiScript(vm, source);
    const second = await executeOpenApiScript(vm, { ...source, request: first.request, variables: first.variables, localVariables: first.localVariables, scopes: first.scopes, script: 'pm.test("temporary override",()=>pm.expect(pm.variables.get("name")).to.equal("temporary"));' });
    expect(second.tests[0]?.passed).toBe(true);
    expect(second.scopes?.environment.name).toBe("environment");
  });
  it("records a synchronous throw inside a done-style assertion as a test failure", async () => {
    const result = await executeOpenApiScript(vm, input('pm.test("callback throws", done => { throw new Error("bad assertion"); }); pm.test("continues",()=>pm.expect(true).to.be.true);'));
    expect(result.tests.map((test) => test.passed)).toEqual([false, true]);
    expect(result.tests[0]?.error).toBe("bad assertion");
  });
  it("removes generated headers and tracks their suppression for the transport", async () => {
    const source = input('pm.request.headers.remove("Accept");');
    source.request.headers.push({ name: "Accept", value: "*/*", enabled: true });
    const result = await executeOpenApiScript(vm, source);
    expect(result.request.disabledHeaders).toContain("accept");
    expect(result.request.headers.some((row) => row.name === "Accept")).toBe(false);
  });
  it("supports form fields and URL encoded request updates", async () => {
    const source = input('pm.request.body.urlencoded.upsert({key:"name",value:"new value"});pm.request.body.urlencoded.remove("old");');
    source.request.mediaType = "application/x-www-form-urlencoded";
    source.request.body = "old=1&name=before";
    const result = await executeOpenApiScript(vm, source);
    expect(result.request.body).toBe("name=new%20value");
    expect(result.request.form).toEqual([{ name: "name", value: "new value", enabled: true }]);
  });
  it("waits for callback and promise HTTP requests and async assertions", async () => {
    const received: string[] = [];
    const result = await executeOpenApiScript(
      vm,
      input(`
      pm.sendRequest("https://api.test/auth",(err,res)=>{if(err)throw err;pm.request.headers.upsert({key:"X-Id",value:res.json().id});});
      const res=await pm.sendRequest({url:"{{baseUrl}}/second",method:"POST",body:{mode:"raw",raw:"hello"}});
      pm.test("async assertion", async()=>{await new Promise(resolve=>setTimeout(resolve,5));pm.expect(res.code).to.equal(200);});
      pm.test("callback assertion", done=>setTimeout(()=>done(),5));
    `),
      {
        sendRequest: async (request) => {
          received.push(request.url);
          return { status: 200, statusText: "OK", duration: 5, headers: [], body: '{"id":42}', truncated: false };
        },
      },
    );
    expect(received).toEqual(["https://api.test/auth", "https://api.test/second"]);
    expect(result.request.headers).toContainEqual({ name: "X-Id", value: "42", enabled: true });
    expect(result.tests.every((test) => test.passed)).toBe(true);
    expect(result.tests).toHaveLength(2);
  });
  it("reports asynchronous callback errors", async () => {
    await expect(executeOpenApiScript(vm, input('pm.sendRequest("https://api.test",()=>{throw new Error("callback failed");});'), { sendRequest: async () => ({ status: 200, statusText: "OK", duration: 1, headers: [], body: "", truncated: false }) })).rejects.toThrow("callback failed");
  });
  it("cancels timers without waiting for their original delay", async () => {
    const result = await executeOpenApiScript(vm, input('const id=setTimeout(()=>{throw new Error("should not run");},30000);clearTimeout(id);console.log("done");'));
    expect(result.logs).toEqual(["done"]);
  });
  it("validates schemas and supports response mutations", async () => {
    const result = await executeOpenApiScript(
      vm,
      input(
        `
      pm.test("schema",()=>pm.response.to.have.jsonSchema({type:"object",required:["id"],properties:{id:{type:"integer"}}}));
      pm.response.setBody('{"changed":true}');
      pm.response.headers.upsert({key:"X-Local",value:"yes"});
    `,
        true,
      ),
    );
    expect(result.tests[0]?.passed).toBe(true);
    expect(result.response?.body).toBe('{"changed":true}');
    expect(result.response?.headers).toContainEqual(["X-Local", "yes"]);
  });
  it("interrupts infinite loops within a bounded execution budget", async () => {
    await expect(executeOpenApiScript(vm, input("while(true){}"))).rejects.toThrow(/interrupted|timeout/);
  }, 10000);
});
