/** Postman-compatible API evaluated inside QuickJS, never in the page's realm.
 * Reference: https://docs.apifox.com/postman-script-api.md and
 * https://docs.apifox.com/access-modify-request-data.md (2026-11-18).
 */
export const OPENAPI_SCRIPT_API = String.raw`
const seed = JSON.parse(__specx_input());
const request = seed.request;
const environment = {...(seed.scopes?.environment ?? seed.variables)};
const globals = {...seed.scopes?.globals};
const local = {...(seed.localVariables ?? Object.fromEntries(Object.entries(seed.variables).filter(([key]) => !Object.hasOwn(environment,key) && !Object.hasOwn(globals,key))))};
const tasks = new Set();
const taskErrors = [];
const track = promise => { tasks.add(promise); promise.then(() => tasks.delete(promise), error => {tasks.delete(promise);taskErrors.push(error);}); return promise; };
const stringify = value => typeof value === 'string' ? value : JSON.stringify(value);
const variableKey = name => {
  name = String(name);
  if (['__proto__','constructor','prototype'].includes(name) || /(?:authorization|cookie|password|passwd|secret|token|api[-_]?key|credential|signature|privatekey)/i.test(name)) throw new Error('script_variable_restricted: ' + name);
  return name;
};
const allVariables = () => ({...globals, ...environment, ...local});
const replaceIn = value => {const result=JSON.parse(__specx_expandVariables(String(value), JSON.stringify(allVariables())));for(const [name,value] of Object.entries(result.updates))environment[variableKey(name)]=value;return result.value;};
const scope = (values, resolved = false) => ({
  get: name => (resolved ? allVariables() : values)[name],
  has: name => Object.hasOwn(resolved ? allVariables() : values, name),
  set: (name, value) => { values[variableKey(name)] = stringify(value); },
  unset: name => { delete values[variableKey(name)]; },
  clear: () => { for (const name of Object.keys(values)) delete values[name]; },
  toObject: () => ({...(resolved ? allVariables() : values)}), replaceIn,
  replaceInAsync: async value => replaceIn(value)
});
const list = (read, write, insensitive = false) => {
  const key = row => String(row.key ?? row.name ?? '');
  const matches = (row, name) => insensitive ? key(row).toLowerCase() === String(name).toLowerCase() : key(row) === String(name);
  const normalized = row => ({key:key(row),value:String(row.value ?? ''),disabled:row.disabled === true || row.enabled === false});
  const api = {
    get: name => read().find(row => matches(row,name) && !normalized(row).disabled)?.value,
    has: name => api.get(name) !== undefined,
    add: row => { write([...read().map(normalized),normalized(row)]); },
    upsert: row => { const rows = read().map(normalized); const index = rows.findIndex(item => matches(item,key(row))); if(index < 0) rows.push({...normalized(row),disabled:false}); else rows[index] = {...normalized(row),disabled:false}; write(rows); },
    remove: name => { write(read().filter(row => !matches(row,name)).map(normalized)); },
    clear: () => write([]),
    all: () => read().map(normalized),
    toJSON: () => read().map(normalized),
    toObject: () => Object.fromEntries(read().filter(row=>!normalized(row).disabled).map(row=>[key(row),row.value])),
    each: callback => read().map(normalized).forEach(callback),
    count: () => read().length,
    get size() {return read().length;}
  };
  return api;
};
let warnedRequestPhase = false;
const requestEdit = () => { if(seed.kind === 'postResponse' && !warnedRequestPhase) { warnedRequestPhase=true; __specx_log('[warn] ' + (seed.messages?.requestAlreadySent ?? 'script_request_already_sent')); } };
const headers = list(() => request.headers, rows => {
  requestEdit();
  const retained=new Set(rows.map(row=>row.key.toLowerCase()));
  request.disabledHeaders=[...new Set([...(request.disabledHeaders ?? []),...request.headers.filter(row=>!retained.has(row.name.toLowerCase())).map(row=>row.name.toLowerCase())])].filter(name=>!rows.some(row=>row.key.toLowerCase()===name && !row.disabled));
  request.headers = rows.map(row=>({name:row.key,value:row.value,enabled:!row.disabled}));
}, true);
let urlValue = request.url;
const parts = () => JSON.parse(__specx_urlParts(urlValue));
const query = list(() => parts().query, rows => {
  requestEdit(); const value = parts(); const queryString = rows.filter(row=>!row.disabled).map(row=>encodeURIComponent(row.key)+'='+encodeURIComponent(row.value)).join('&');
  urlValue = value.base + (queryString ? '?' + queryString : '') + value.hash;
});
const url = { query, toString:()=>urlValue, toJSON:()=>urlValue, update:value=>{requestEdit();urlValue=String(value);}, getHost:()=>parts().hostname, getPath:()=>parts().pathname, getQueryString:()=>urlValue.split('?')[1]?.split('#')[0] ?? '' };
const parseForm = () => request.form ?? (request.form = request.mediaType === 'application/x-www-form-urlencoded' ? request.body.split('&').filter(Boolean).map(item=>{const [key,...rest]=item.split('=');return {name:decodeURIComponent(key.replace(/\+/g,' ')),value:decodeURIComponent(rest.join('=').replace(/\+/g,' ')),enabled:true};}) : []);
const form = list(parseForm, rows => { requestEdit();request.form = rows.map(row=>({name:row.key,value:row.value,enabled:!row.disabled})); request.body = rows.filter(row=>!row.disabled).map(row=>encodeURIComponent(row.key)+'='+encodeURIComponent(row.value)).join('&'); });
const body = {
  get mode(){return request.mediaType === 'multipart/form-data' ? 'formdata' : request.mediaType === 'application/x-www-form-urlencoded' ? 'urlencoded' : 'raw';},
  get raw(){return request.body;}, set raw(value){requestEdit();request.body=String(value);},
  formdata:form, urlencoded:form,
  toString:()=>request.body, toJSON:()=>({mode:body.mode,raw:request.body,formdata:form.toJSON(),urlencoded:form.toJSON()}),
  update:value=>{ requestEdit();if(typeof value === 'string') request.body=value; else if(value.mode === 'urlencoded' || value.mode === 'formdata') { request.mediaType=value.mode === 'formdata' ? 'multipart/form-data' : 'application/x-www-form-urlencoded'; form.clear(); for(const row of value[value.mode] ?? []) form.add(row); } else request.body=String(value.raw ?? ''); }
};
const requestApi = {
  headers,body,
  get:()=>JSON.parse(JSON.stringify({...request,url:urlValue})),
  get url(){return url;}, set url(value){requestEdit();urlValue=String(value);},
  get method(){return request.method;}, set method(value){requestEdit();request.method=String(value).toUpperCase();},
  get auth(){return request.auth;},
  getBaseUrl:()=>seed.info?.baseUrl ?? parts().origin,
  setHeader:(name,value)=>headers.upsert({key:String(name),value:String(value)}),
  removeHeader:name=>headers.remove(name), setBody:value=>body.update(value)
};
const responseApi = value => {
  if(!value) return undefined;
  const response = {
    code:value.status, status:value.statusText, statusText:value.statusText, responseCode:value.status, responseTime:value.duration,
    get responseSize(){return value.body.length;},
    get body(){return value.body;},
    text:()=>value.body, json:()=>JSON.parse(value.body), setBody:text=>{value.body=String(text);},
    headers:list(()=>value.headers.map(([key,value])=>({key,value})),rows=>{value.headers=rows.map(row=>[row.key,row.value]);},true),
  };
  const have = {
    status:code=>__specx_expect(typeof code === 'number' ? response.code : response.status).to.equal(code),
    header:(key,expected)=>{__specx_expect(response.headers.has(key)).to.equal(true);if(expected!==undefined)__specx_expect(response.headers.get(key)).to.equal(expected);},
    body:expected=>{ if(expected instanceof RegExp) __specx_expect(value.body).to.match(expected); else if(expected===undefined) __specx_expect(value.body).not.to.equal(''); else __specx_expect(value.body).to.equal(expected); },
    jsonBody:(path,expected)=>{ const data=response.json(); if(typeof path==='string') {const assertion=__specx_expect(data).to.have.nested.property(path);if(expected!==undefined)assertion.to.deep.equal(expected);} else if(path!==undefined)__specx_expect(data).to.deep.equal(path);},
    jsonSchema:schema=>{ const result=JSON.parse(__specx_validateSchema(JSON.stringify(schema),value.body)); __specx_expect(result.errors.join('; ')).to.equal(''); }
  };
  const be={};
  const ranges={info:[100,199],success:[200,299],redirection:[300,399],clientError:[400,499],serverError:[500,599],error:[400,599],ok:[200,200],accepted:[202,202],badRequest:[400,400],unauthorized:[401,401],forbidden:[403,403],notFound:[404,404],rateLimited:[429,429]};
  for(const [name,[min,max]] of Object.entries(ranges))Object.defineProperty(be,name,{get:()=>__specx_expect(response.code).to.be.within(min,max)});
  response.to={have,be}; return response;
};
let testCount=0;
const test=(name,fn)=>{
  const record=error=>__specx_test(JSON.stringify({name:String(name),passed:!error,...(error?{error:String(error.message ?? error)}:{})}));
  testCount++;
  try {
    if(fn.length) {track(new Promise(resolve=>{let doneCalled=false;const done=error=>{if(doneCalled)return;doneCalled=true;record(error);resolve();};try{fn(done);}catch(error){done(error);}}));}
    else {const result=fn(); if(result && typeof result.then === 'function')track(Promise.resolve(result).then(()=>record(),record));else record();}
  } catch(error){record(error);}
  return pm;
};
test.index=()=>testCount;
const consoleApi={};
for(const level of ['log','info','debug','warn','error'])consoleApi[level]=(...values)=>__specx_log((level==='log'?'':'['+level+'] ')+values.map(value=>{try{return stringify(value) ?? String(value);}catch{return String(value);}}).join(' '));
const sendRequest=(value,callback)=>{
  const input=typeof value === 'string'?{url:value}:value;
  const rawHeaders=input.header ?? input.headers ?? {};
  const entries=Array.isArray(rawHeaders)?rawHeaders.filter(row=>!row.disabled).map(row=>[row.key ?? row.name,row.value]):Object.entries(rawHeaders);
  const payload={url:replaceIn(String(input.url)),method:String(input.method ?? 'GET').toUpperCase(),headers:entries.map(([k,v])=>[k,replaceIn(v)])};
  if(input.body !== undefined) {
    if(typeof input.body === 'string')payload.body=replaceIn(input.body);
    else if(input.body.mode === 'urlencoded') {payload.body=(input.body.urlencoded ?? []).filter(row=>!row.disabled).map(row=>encodeURIComponent(row.key)+'='+encodeURIComponent(replaceIn(row.value))).join('&');payload.mediaType='application/x-www-form-urlencoded';}
    else if(input.body.mode === 'raw' || !input.body.mode)payload.body=replaceIn(input.body.raw ?? '');
    else throw new Error('script_http_body_unsupported: '+input.body.mode);
  }
  const promise=__specx_sendRequest(JSON.stringify(payload)).then(json=>{const result=responseApi(JSON.parse(json));if(callback)callback(null,result);return result;},error=>{if(callback){callback(error);return;}throw error;});
  return track(promise);
};
const pm={
  info:{eventName:seed.kind==='preRequest'?'prerequest':'test',iteration:0,iterationCount:1,requestName:seed.info?.requestName ?? request.url,requestId:seed.info?.requestId ?? ''},
  variables:scope(local,true), environment:scope(environment), globals:scope(globals), iterationData:scope({}),
  request:requestApi,response:responseApi(seed.response),expect:__specx_expect,test,sendRequest,
  visualizer:{set:(template,data)=>__specx_visualize(String(template),JSON.stringify(data ?? {})),clear:()=>__specx_visualize('', '{}')},
};
const cookies={};
for(const [name,value] of seed.response?.headers ?? []) if(name.toLowerCase()==='set-cookie') {const pair=value.split(';')[0];const index=pair.indexOf('=');if(index>0)cookies[pair.slice(0,index).trim()]=pair.slice(index+1);}
pm.cookies={get:key=>cookies[key],has:key=>Object.hasOwn(cookies,key),toObject:()=>({...cookies})};
const specx={env:pm.environment,environment:pm.environment,variables:pm.variables,request:requestApi,response:seed.response ? {...seed.response,json:(()=>{try{return JSON.parse(seed.response.body);}catch{return null;}})()} : null,console:consoleApi,log:consoleApi.log,test,expect:pm.expect,sendRequest};
if(!seed.response)Object.defineProperty(pm,'response',{get(){throw new Error('pm.response is only available in post-response scripts');}});
Object.assign(globalThis,{pm,specx,hopp:{...specx,response:responseApi(seed.response)},console:consoleApi});
let timerId=0;
const timers=new Map();
globalThis.setTimeout=(callback,ms=0,...args)=>{
  const id=++timerId;timers.set(id,true);
  track(__specx_delay(Math.max(0,Number(ms)||0),id).then(()=>{if(timers.has(id)){timers.delete(id);return callback(...args);}}));return id;
};
globalThis.clearTimeout=id=>{timers.delete(id);__specx_cancelDelay(id);};
globalThis.__specx_drain=async()=>{while(tasks.size)await Promise.all([...tasks]);if(taskErrors.length)throw taskErrors[0];};
globalThis.__specx_finish=()=>{request.url=urlValue;__specx_commit(JSON.stringify({request,variables:allVariables(),localVariables:local,scopes:{environment,globals},response:seed.response}));};
`;
