// Actual New Song handlers, with no native window or production filesystem access.
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const nodes = new Map(), calls = [];
function node(id) {
  if (!nodes.has(id)) nodes.set(id, {
    value: "", textContent: "", listeners: {},
    addEventListener(event, handler) { this.listeners[event] = handler; },
    classList: { toggle() {} }, setAttribute() {}, focus() {},
  });
  return nodes.get(id);
}
let production = "/production/media", picked = "/explicit-test-root";
const context = vm.createContext({
  $:node, console,
  document: {addEventListener() {}},
  window: {__TAURI__: {dialog: {open:async () => picked}}},
  invoke:async (id) => { calls.push(id); assert.equal(id,"projects_root"); return production; },
  toast:message => { throw Error(message); },
});
vm.runInContext(readFileSync(new URL("../ui/home.js",import.meta.url),"utf8"),context);
await vm.runInContext("newSheet(true)",context);
assert.equal(node("new-root").value,production);
assert.match(node("new-where").textContent,/Production root/);
await node("new-root-pick").listeners.click();
assert.equal(node("new-root").value,picked);
assert.match(node("new-where").textContent,/One-project override/);
assert.ok(node("new-where").textContent.includes(production));
await vm.runInContext("newSheet(false); newSheet(true)",context);
assert.equal(node("new-root").value,production,"a cancelled test root must not become the next default");
production="/changed-production/media";
await vm.runInContext("newSheet(true)",context);
assert.equal(node("new-root").value,production,"read the shared setting on every New Song request");
assert.ok(calls.every(id=>id==="projects_root"),"choosing an override must not save global settings or read recents");
console.log("PASS: New Song uses the shared root, labels one-project overrides, and resets them on reopen");
