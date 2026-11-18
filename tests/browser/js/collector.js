// What must still be there after the collector has run: everything
// here is made, left alone while timers make garbage, and looked at
// again at the end. The test runs it a second time with
// --collect-always, which collects after every timer.
var log = [];
function L() { console.log(Array.prototype.join.call(arguments, " | ")); }
function later(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }
function garbage(n) {
  var a = [];
  for (var i = 0; i < n; i++) a.push({ i: i, s: "x" + i, f: function () { return i; }, l: [i, [i]] });
  return a.length;
}

// closures, each over its own variables
function counter() { var n = 0; return { up: function () { n++; return n; }, get: function () { return n; } }; }
var c1 = counter();

// a generator stopped inside a try and a block, holding objects only it knows
function* gen() {
  var kept = { v: 1 };
  try {
    for (let i = 0; i < 3; i++) { let boxed = { i: i }; yield function () { return boxed.i + kept.v; }; }
  } finally { log.push("gen done"); }
}
var g = gen();
var fns = [g.next().value];

// an async function waiting on a timer, with objects of its own
async function slow(name) {
  var mine = { name: name, list: [1, 2, 3] };
  await later(5);
  garbage(2000);
  await later(5);
  return mine.name + mine.list.length;
}

// collections with objects as keys
var m = new Map(), s = new Set(), wm = new WeakMap(), keys = [];
for (var i = 0; i < 50; i++) { var k = { id: i }; keys.push(k); m.set(k, { v: i }); s.add(k); wm.set(k, { w: i * 2 }); }
// an entry whose value points back at its key, and nobody else holds the key: both go, quietly
(function () { var k2 = {}; wm.set(k2, { back: k2 }); })();
// what one WeakMap holds is the key of the next
var wa = new WeakMap(), wb = new WeakMap(), head = { name: "head" };
(function () { var mid = { name: "mid" }; wa.set(head, mid); wb.set(mid, { name: "tail" }); })();

// a symbol as a key, a getter, a bound function, a proxy, bytes
var sym = Symbol("tag");
var holder = {};
holder[sym] = { deep: [1, 2, { three: 3 }] };
Object.defineProperty(holder, "got", { get: function () { return this[sym].deep[2].three; } });
var bound = function (a, b) { return this.base + a + b; }.bind({ base: 100 }, 20);
var target = { secret: { n: 7 } };
var proxy = new Proxy(target, { get: function (t, k) { return k === "n" ? t.secret.n : t[k]; } });
var bytes = new Uint8Array(new ArrayBuffer(8), 2, 4);
bytes[0] = 9;
var view = new DataView(bytes.buffer);

// a long chain: finding it alive must not go one call deeper for each link
class Link { constructor(v, next) { this.v = v; this.next = next; } static sum(n) { var t = 0; while (n) { t += n.v; n = n.next; } return t; } }
var chain = null;
for (var j = 1; j <= 20000; j++) chain = new Link(j, chain);

var re = /(\d+)-(\d+)/g, date = new Date(86400000), err = new RangeError("kept");
var cyc = { name: "a" };
cyc.self = cyc;
cyc.other = { back: cyc };
var it = [10, 20, 30][Symbol.iterator]();
it.next();
var registry = Symbol.for("shared");
var pending = new Promise(function (res) { setTimeout(function () { res({ late: true }); }, 30); });
var ref = new WeakRef(keys[0]);
function tagged(strings) { return strings; }
function tag() { return tagged`a${1}b`; }
var first = tag();

var tick = 0;
var timer = setInterval(function () {
  tick++;
  garbage(3000);
  fns.push(g.next().value);
  if (tick == 3) { clearInterval(timer); finish(); }
}, 4);

async function finish() {
  var a = await slow("first"), b = await Promise.all([slow("x"), slow("y")]);
  var late = await pending;
  L(c1.up(), c1.up(), c1.get());
  L(fns.filter(Boolean).map(function (f) { return f(); }).join(","), g.next().done, log.join());
  L(a, b.join(), late.late);
  L(m.size, s.size, keys.every(function (k, i) { return m.get(k).v === i && s.has(k) && wm.get(k).w === i * 2; }));
  L(wa.get(head).name, wb.get(wa.get(head)).name);
  L(holder.got, holder[sym].deep.length, Object.getOwnPropertySymbols(holder).length, sym.toString(), Symbol.for("shared") === registry);
  L(bound(3), proxy.n, proxy.secret.n, bytes[0], view.getUint8(2), bytes.buffer.byteLength);
  L(Link.sum(chain), "12-34 5-6".replace(re, "$2-$1"), date.toISOString(), err.message, err instanceof RangeError);
  L(cyc.self.other.back.name, it.next().value, ref.deref() === keys[0], tag() === first, first.raw.join());
  L(typeof setTimeout, [1, 2, 3].map(function (x) { return x * 2; }).join(), JSON.stringify({ a: [1, { b: 2 }] }), new Map([[1, 2]]).get(1));
  L("done");
}
