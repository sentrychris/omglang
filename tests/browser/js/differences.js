// Where this engine knowingly differs from the standard. The answers
// in want/differences.txt are this engine's own; what a standard
// engine prints is in the comment on each line.

// An array has no holes: the gap is filled with undefined.
var b = [1, 2, 3]; b.length = 1; b[3] = 4;
console.log(b.hasOwnProperty(1), Object.keys(b).join());            // false 0,3

// A property can be hidden from loops one at a time, but read-only
// is for a whole object (Object.freeze), not for one property.
var o = {}; Object.defineProperty(o, "h", { value: 1 });
o.h = 2;
console.log(o.h, Object.keys(o).length);                            // 1 0

// A string is a list of code points, not of UTF-16 units.
console.log("😀".length, "😀".charCodeAt(0));                         // 2 55357

// There are no big integers, and a regular expression is not asked
// for its Symbol.replace or Symbol.split.
console.log(typeof BigInt, typeof /a/[Symbol.replace]);             // function function

// Memory is collected between two pieces of work, never in the middle
// of one, and as soon as a hundred thousand objects were made since
// the last time: a WeakRef whose object nothing else holds is empty
// in the next timer, and what a WeakMap held for it is gone. Node
// promises neither when nor whether.
var lost = new WeakRef({ gone: true }), kept = { here: true }, held = new WeakRef(kept);
var notes = new WeakMap([[kept, "about kept"]]);
for (var filler = 0; filler < 120000; filler++) ({ n: filler });
setTimeout(function () {
  console.log(typeof WeakRef, lost.deref(), held.deref() === kept, notes.get(kept));   // function undefined true about kept
}, 0);
