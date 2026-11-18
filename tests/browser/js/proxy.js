var log = [];
var t = {a: 1, get b() { return this.a + 1; }};
var p = new Proxy(t, {
  get(target, key, recv) { log.push('get ' + String(key)); return Reflect.get(target, key, recv); },
  set(target, key, v, recv) { log.push('set ' + key); return Reflect.set(target, key, v, recv); },
  has(target, key) { log.push('has ' + key); return key in target; },
  deleteProperty(target, key) { log.push('del ' + key); return delete target[key]; },
  ownKeys(target) { log.push('keys'); return Reflect.ownKeys(target); }
});
console.log(p.a, p.b, 'a' in p, 'z' in p);
p.c = 3; p.a = 5;
console.log(JSON.stringify(t), Object.keys(p).join(), delete p.c, JSON.stringify(p));
for (var k in p) log.push('forin ' + k);
console.log(log.join(' | '));
var arr = new Proxy([1, 2, 3], {});
console.log(Array.isArray(arr), arr.length, arr.map(function (x) { return x * 2; }).join(), JSON.stringify(arr), arr instanceof Array);
arr.push(4); console.log(arr.join(), [...arr].length, Object.prototype.toString.call(arr));
var calls = [];
var arr2 = new Proxy([], { set(t, k, v) { calls.push(k + '=' + v); t[k] = v; return true; } });
arr2.push('x', 'y'); console.log(calls.join(), arr2.length);
var f = new Proxy(function (a, b) { return a + b; }, { apply(target, thisArg, args) { return target.apply(thisArg, args) * 10; } });
console.log(f(1, 2), typeof f, typeof p);
var C = new Proxy(function () { this.x = 1; }, { construct(target, args) { return {made: args.length}; } });
console.log(JSON.stringify(new C(1, 2)));
var r = Proxy.revocable({q: 1}, {});
console.log(r.proxy.q); r.revoke();
try { r.proxy.q; } catch (e) { console.log(e instanceof TypeError); }
var sym = Symbol('s');
var ps = new Proxy({[sym]: 9}, { get(t, k) { return typeof k === 'symbol' ? 'sym:' + (k === sym) : t[k]; } });
console.log(ps[sym], ps[Symbol.iterator]);
var m = new Proxy(new Map([[1, 2]]), { get(t, k) { var v = Reflect.get(t, k, t); return typeof v === 'function' ? v.bind(t) : v; } });
console.log(m.get(1), m.size, m instanceof Map);
var d = new Proxy({}, { defineProperty(t, k, desc) { log.length = 0; log.push('def ' + k); return Reflect.defineProperty(t, k, desc); },
  getOwnPropertyDescriptor(t, k) { return Reflect.getOwnPropertyDescriptor(t, k); } });
Object.defineProperty(d, 'x', {value: 1, enumerable: true, configurable: true, writable: true});
console.log(log.join(), d.x, Object.getOwnPropertyDescriptor(d, 'x').value, d.hasOwnProperty('x'));
with (new Proxy({}, { has(t, k) { return k === 'magic'; }, get(t, k) { return k === 'magic' ? 42 : undefined; } })) { console.log(magic, typeof log); }
var nested = new Proxy(new Proxy({v: 1}, {}), {}); console.log(nested.v, Object.assign({}, nested).v, {...nested}.v);
