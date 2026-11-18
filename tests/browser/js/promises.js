var out = [];
var L = function () { out.push(Array.prototype.map.call(arguments, function (x) { return typeof x === "string" ? x : JSON.stringify(x) }).join(" | ")) };
var late = {}; var T = function (name) { late[name] = Array.prototype.slice.call(arguments, 1).map(function (x) { return typeof x === "string" ? x : JSON.stringify(x) }).join(" | ") };
L("start");
Promise.resolve(1).then(function (v) { L("then", v); return v + 1 }).then(function (v) { L("then2", v); throw new Error("boom") }).catch(function (e) { L("caught", e.message); return "rec" }).finally(function () { L("finally") }).then(function (v) { L("after", v) });
new Promise(function (res, rej) { L("executor"); res("a"); rej("b"); res("c") }).then(function (v) { L("p", v) });
new Promise(function () { throw new TypeError("in executor") }).catch(function (e) { L(e.name, e.message) });
Promise.reject(3).then(function () { L("no") }, function (r) { L("rejected", r) });
queueMicrotask(function () { L("micro") });
setTimeout(function () { T("timeout 0", "ran") }, 0);
setTimeout(function (a, b) { T("timeout 20", a, b) }, 20, "x", "y");
var n = 0; var iv = setInterval(function () { n++; if (n == 3) { clearInterval(iv); T("ticks", n) } }, 5);
var dead = setTimeout(function () { L("never") }, 1); clearTimeout(dead);
Promise.all([1, Promise.resolve(2), new Promise(function (r) { setTimeout(r, 10, 3) })]).then(function (v) { T("all", v) });
Promise.all([]).then(function (v) { L("all empty", v) });
Promise.all([Promise.reject("r1"), 2]).catch(function (e) { L("all rejected", e) });
Promise.allSettled([1, Promise.reject("x")]).then(function (v) { L("settled", v) });
Promise.race([new Promise(function (r) { setTimeout(r, 30, "slow") }), new Promise(function (r) { setTimeout(r, 2, "fast") })]).then(function (v) { T("race", v) });
Promise.any([Promise.reject(1), Promise.resolve(2)]).then(function (v) { L("any", v) });
Promise.any([Promise.reject(1)]).catch(function (e) { L("any none", e.name, e.errors, e.message) });
async function f1(x) { L("f1 start", x); var a = await x; L("f1 mid", a); var b = await Promise.resolve(a + 1); L("f1 b", b); try { await Promise.reject(new Error("aw")) } catch (e) { L("f1 caught", e.message) } return b * 2 }
f1(10).then(function (v) { L("f1 done", v) });
async function f2() { throw new RangeError("f2") } f2().catch(function (e) { L("f2", e.name) });
var f3 = async () => { await null; return "arrow" }; f3().then(function (v) { L(v) });
class K { async m(x) { await 0; return this.v + x } constructor() { this.v = 5 } } new K().m(1).then(function (v) { L("method", v) });
var thenable = { then: function (res) { L("thenable then"); res("from thenable") } };
Promise.resolve(thenable).then(function (v) { L(v) });
(async function () { L("await thenable", await thenable) })();
var order = []; Promise.resolve().then(function () { order.push(1) }).then(function () { order.push(3) }).then(function () { order.push(5) }); Promise.resolve().then(function () { order.push(2) }).then(function () { order.push(4) }).then(function () { L("order", order) });
(async function () { var p1 = Promise.resolve("p1"); order.push("a"); await p1; order.push("b"); await p1; order.push("c") })(); 
var p = Promise.resolve(7); L(Promise.resolve(p) === p, p instanceof Promise, typeof p.then, Object.prototype.toString.call(p), Promise.length, Promise.name);
new Promise(function (res) { res(Promise.resolve("nested")) }).then(function (v) { L(v) });
Promise.resolve(1).finally(function () { return Promise.reject("fin") }).catch(function (e) { L("finally rejects", e) });
Promise.reject("keep").finally(function () { return 1 }).catch(function (e) { L("finally keeps", e) });
var wr = Promise.withResolvers ? Promise.withResolvers() : null; if (wr) { wr.promise.then(function (v) { L("withResolvers", v) }); wr.resolve("wr") } else L("withResolvers", "wr");
async function loop() { var s = 0; for (var i = 0; i < 5; i++) { s += await new Promise(function (r) { setTimeout(r, 1, i) }) } return s } loop().then(function (v) { T("loop", v) });

function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms) }) }
(async function () { await sleep(15); T("slept", "yes"); var r = await Promise.all([sleep(5).then(function () { return "a" }), "b"]); T("after sleep", r); var seen = []; for await (var x of [Promise.resolve(1), 2]) seen.push(x); T("for await", seen) })();
try { Promise.resolve().then.call(1) } catch (e) { L(e.name) } try { Promise() } catch (e) { L(e.name) } try { new Promise(1) } catch (e) { L(e.name) }
var self = new Promise(function (r) { setTimeout(function () { r(self) }, 1) }); self.catch(function (e) { T("cycle", e.name) });
L("end of script");
setTimeout(function () { Object.keys(late).sort().forEach(function (k) { out.push(k + ": " + late[k]) }); console.log(out.join("\n")) }, 500);
