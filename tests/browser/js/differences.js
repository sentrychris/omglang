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

// The weak collections hold on to their keys.
console.log(typeof WeakRef, new WeakRef(o).deref() === o);          // function true (the same, until a collector runs)
