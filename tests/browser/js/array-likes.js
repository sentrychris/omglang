var o = {length: 0};
console.log(Array.prototype.push.call(o, 1, 2, 3), JSON.stringify(o));
console.log(Array.prototype.pop.call(o), JSON.stringify(o));
console.log(Array.prototype.splice.call(o, 0, 1, 'a', 'b').join(), JSON.stringify(o));
console.log(Array.prototype.sort.call(o, function(a,b){return String(a)<String(b)?1:-1}) === o, JSON.stringify(o));
console.log(Array.prototype.reverse.call(o) === o, JSON.stringify(o));
console.log(Array.prototype.shift.call(o), JSON.stringify(o));
console.log(Array.prototype.unshift.call(o, 9), JSON.stringify(o));
