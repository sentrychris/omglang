var o = {a: 1, b: 2, f: function () { return this === o; }};
var a = 10, c = 30;
with (o) {
  console.log(a, b, c, f(), typeof a, typeof zz);
  a = 5; c = 31;
  var b = 7;
  let d = a + b;
  console.log(d);
  var g = function (x) { return a + x; };
  a++;
  b += 1;
}
console.log(JSON.stringify(o), a, c, g(1), b);
function t(obj) { with (obj) { with ({y: 2}) { return x + y; } } }
console.log(t({x: 1}), t({x: 5, y: 9}));
var render = new Function('obj', "var __p=''; with(obj||{}){ __p+='hello '+name+'!'; for (var i=0;i<n;i++){ __p+='*'; } } return __p;");
console.log(render({name: 'omg', n: 3}));
for (var k = 0; k < 2; k++) with ({q: k}) { if (q == 0) continue; console.log('q', q); }
const _Vue = { vModelText: {m: 1}, wd: function (v, d) { return [v, d]; }, h: function (t) { return 'h:' + t; } };
function mk() {
  return function render(_ctx, _cache) {
    with (_ctx) {
      const { vModelText: _vModelText, wd: _wd, h: _h } = _Vue
      return _wd((_h("input")), [[_vModelText, name]])
    }
  }
}
var ctx = new Proxy({name: 'nm'}, { has(t, k) { return k[0] !== '_'; }, get(t, k) { return t[k]; } });
console.log(JSON.stringify(mk()(ctx, [])));
var f = new Function('Vue', 'const _Vue = Vue\nreturn function render(_ctx, _cache) {\n  with (_ctx) {\n    const { vModelText: _vModelText, wd: _wd, h: _h } = _Vue\n\n    return _wd((_h("input")), [\n [_vModelText, name]\n ])\n  }\n}');
console.log(JSON.stringify(f(_Vue)(ctx, [])));
