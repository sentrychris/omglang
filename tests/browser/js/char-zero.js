// The character with code 0 inside strings. A program built through
// the C back end cannot keep one (its strings end at the first NUL),
// so tests/browser.sh runs this script only where omg writes machine
// code itself.
function L() { console.log(Array.prototype.join.call(arguments, " | ")); }

var z = "\0", s = "a\0b";
L(z.length, s.length, s.charCodeAt(1), s.indexOf("b"), s.indexOf("\0"), s === "a\0c", s === "a" + String.fromCharCode(0) + "b");
L(/\0/.test(z), /^a\0b$/.test(s), /[\0]/.test(s), /\x00/.test("abc"), s.split("\0").join("+"), s.replace(/\0/g, "-"));
L(JSON.stringify(s), JSON.parse('"a\\u0000b"') === s, escape(s), encodeURIComponent(s), s.slice(1).length, (s + s).length);
var o = {}; o[s] = 1; o["a\0c"] = 2;
L(Object.keys(o).length, o[s], o["a\0c"], o["a"], new Map([[z, "zero"]]).get("\0"), [z, "a", ""].sort().map(function (x) { return x.length }));
L(new TextEncoder().encode(s).join(), new TextDecoder().decode(new Uint8Array([97, 0, 98])) === s, "\u0000" === z, `\0`.length);
