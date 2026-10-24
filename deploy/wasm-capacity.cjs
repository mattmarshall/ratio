// Run a Bazel-built optimized module in a fresh instance. Durable output uses
// ratio.storage.v1.WasmFoldMeasurement; console output is operator progress.
const fs = require('node:fs');
const crypto = require('node:crypto');
const {execFileSync} = require('node:child_process');
const [modulePath, revision, output, requested, views] = process.argv.slice(2);
if (!modulePath || !/^[0-9a-f]{40}$/.test(revision) || !output ||
    !/^\d+$/.test(requested) || Number(requested) < 1 || Number(requested) > 4294967295 || !['1', '2'].includes(views)) {
  throw new Error('usage: node wasm-capacity.cjs MODULE CLEAN_REVISION OUTPUT.pb HEIGHT VIEWS');
}
if (execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim() !== revision ||
    execFileSync('git', ['status', '--porcelain'], {encoding: 'utf8'}).trim()) {
  throw new Error('measurement requires a clean checkout at the named revision');
}
if (fs.existsSync(output)) throw new Error('refusing to replace measurement');
const bytes = fs.readFileSync(modulePath);
// Read the module's declared memory maximum rather than trusting a label.
let offset = 8, limit;
function readVarint() { let n = 0, shift = 0, b; do { b = bytes[offset++]; n += (b & 127) * 2 ** shift; shift += 7; } while (b & 128); return n; }
while (offset < bytes.length) {
  const section = bytes[offset++], length = readVarint(), end = offset + length;
  if (section === 5) {
    if (readVarint() !== 1 || readVarint() !== 1) throw new Error('expected one bounded, unshared memory');
    readVarint();
    limit = readVarint() * 65536;
  }
  offset = end;
}
if (![1073741824, 2147483648].includes(limit)) throw new Error('unsupported measurement memory profile');
const moduleObject = new WebAssembly.Module(bytes);
let instance, completed = 0, failure = '', returned = false;
instance = new WebAssembly.Instance(moduleObject, {env: {
  ratio_monotonic_nanos: () => process.hrtime.bigint(),
  ratio_fold_progress: height => {
    completed = height;
    if (height % 1000000 === 0) console.log('completed', height,
      'linear_memory_bytes', instance.exports.memory.buffer.byteLength);
  },
}});
const started = process.hrtime.bigint();
try { returned = instance.exports.ratio_wasm_capacity(Number(requested), Number(views)) === Number(requested); }
catch (error) { failure = String(error); }
const peak = instance.exports.memory.buffer.byteLength;
const elapsed = (process.hrtime.bigint() - started) / 1000000n;
function varint(n) { const bytes = []; n = BigInt(n); do { let b = Number(n & 127n); n >>= 7n; bytes.push(b | (n ? 128 : 0)); } while (n); return Buffer.from(bytes); }
function number(field, value) { return Buffer.concat([varint(field * 8), varint(value)]); }
function string(field, value) { const b = Buffer.from(value); return Buffer.concat([varint(field * 8 + 2), varint(b.length), b]); }
fs.writeFileSync(output, Buffer.concat([
  string(1, revision), string(2, crypto.createHash('sha256').update(bytes).digest('hex')),
  string(3, `${process.version}; V8 ${process.versions.v8}`), number(4, requested),
  number(5, views), number(6, completed), number(7, peak), number(8, limit),
  number(9, returned ? 1 : 0), string(10, failure), number(11, elapsed), number(12, 10000),
]), {flag: 'wx'});
console.log({requested: Number(requested), views: Number(views), completed, peak,
  limit, returned, failure, elapsedMilliseconds: Number(elapsed)});
if (!returned && !failure) throw new Error('module returned no figure without a refusal');
