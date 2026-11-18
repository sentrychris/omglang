var L = function () { console.log(Array.prototype.map.call(arguments, function (x) { return typeof x === "string" ? x : JSON.stringify(x) }).join(" | ")) };
var m = new Map([[1, "a"], ["1", "b"], [NaN, "n"]]);
m.set({}, 1).set(-0, "z");
L(m.size, m.get(1), m.get("1"), m.get(NaN), m.get(0), m.has(2), [...m.keys()].length, [...m.values()], m.delete(1), m.delete(1), m.size);
var o1 = {}, o2 = {}; m.set(o1, "o1"); m.set(o2, "o2"); L(m.get(o1), m.get(o2), m.get({}));
var out = []; m.forEach(function (v, k, mm) { out.push(typeof k + ":" + v + (mm === m)) }); L(out);
L([...new Map([["a", 1], ["b", 2]])], [...new Map([["a", 1]]).entries()], Object.fromEntries(new Map([["x", 1]])), new Map(Object.entries({ q: 1 })).get("q"));
var s = new Set([3, 1, 3, 2, 1]); L(s.size, [...s], s.has(3), s.has(4), s.add(4) === s, s.delete(3), [...s], [...s.entries()], [...s.keys()]);
s.clear(); L(s.size, [...s]);
var it = new Map([[1, 2]])[Symbol.iterator](); L(it.next(), it.next(), typeof it[Symbol.iterator]);
for (var [k, v] of new Map([["k1", "v1"], ["k2", "v2"]])) L(k, v);
for (var x of new Set("hello")) L(x);
var ws = new WeakSet(), wm = new WeakMap(), key = {}; ws.add(key); wm.set(key, 5); L(ws.has(key), ws.has({}), wm.get(key), wm.has({}), wm.delete(key), wm.has(key));
try { wm.set(1, 2) } catch (e) { L(e instanceof TypeError) }
L(Object.prototype.toString.call(new Map), Object.prototype.toString.call(new Set), String(new Map), typeof Map.prototype.forEach, Map.name, Set.length, new Map instanceof Map);
try { Map() } catch (e) { L(e.name) }
class M2 extends Map { constructor(x) { super(x); this.extra = 1 } twice(k) { return this.get(k) * 2 } }
var m2 = new M2([["a", 21]]); L(m2.twice("a"), m2 instanceof Map, m2.size, m2.extra);
var big = new Map(); for (var i = 0; i < 5000; i++) big.set("k" + i, i); for (var i = 0; i < 5000; i += 2) big.delete("k" + i); L(big.size, big.get("k4999"), big.get("k4998"), [...big.keys()].slice(0, 3));
var ms = new Set(); ms.add(1); ms.add(2); var seen = []; for (var q of ms) { seen.push(q); if (q < 4) ms.add(q + 2) } L(seen.length >= 2);
// Date
var d = new Date(Date.UTC(2024, 1, 29, 13, 45, 30, 123));
L(d.getTime(), d.toISOString(), d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), d.getUTCDay(), d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds(), d.getUTCMilliseconds());
L(JSON.stringify(d), d.toJSON(), d.valueOf() === +d, typeof Date.now(), Date.now() > 1.7e12, new Date(0).toISOString(), new Date(-1).toISOString(), new Date(86400000 * 365.25 * 100).toISOString());
L(new Date("2020-02-03T04:05:06Z").getTime(), new Date("2020-02-03T04:05:06.789+02:00").getTime(), new Date("2020-02-03").getTime(), Date.parse("1970-01-01T00:00:00.000Z"), isNaN(new Date("rubbish")), String(new Date(NaN)), new Date(NaN).getTime());
L(Date.UTC(1970, 0, 1), Date.UTC(2000, 11, 31, 23, 59, 59), Date.UTC(1969, 11, 31), Date.UTC(2024, 12, 1), Date.UTC(2024, -1, 1), Date.UTC(99, 0), Date.UTC(1600, 1, 29));
var e = new Date(Date.UTC(2021, 0, 31)); e.setUTCMonth(1); L(e.toISOString()); e.setUTCDate(0); L(e.toISOString()); e.setUTCFullYear(2000, 1, 29); L(e.toISOString()); e.setUTCHours(25, 61, 61, 1001); L(e.toISOString()); e.setTime(1e12); L(e.toISOString(), e.toUTCString());
L(new Date(2e12) - new Date(1e12), new Date(1e12) < new Date(2e12), new Date(5).getTime(), new Date(new Date(7)).getTime(), new Date("Tue, 03 Feb 2020 04:05:06 GMT").getTime(), new Date(8.64e15 + 1).getTime());
L(Object.prototype.toString.call(d), typeof Date(), Date.length, d instanceof Date, new Date(Date.UTC(-1, 0)).toISOString(), new Date(Date.UTC(10000, 0)).toISOString());
try { new Date(NaN).toISOString() } catch (e2) { L(e2.name) }
