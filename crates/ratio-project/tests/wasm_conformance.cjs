// Host runner for the Bazel-built kernel. JSON here is the existing reusable
// fixture's result shape, compared byte-for-byte with the native JSONL path.
const fs = require('node:fs');
const assert = require('node:assert/strict');
const moduleBytes = fs.readFileSync(process.argv[2]);
const moduleObject = new WebAssembly.Module(moduleBytes);
const instance = new WebAssembly.Instance(moduleObject, {env: {
  ratio_monotonic_nanos: () => process.hrtime.bigint(),
  ratio_fold_progress: () => {},
}});
assert.equal(instance.exports.ratio_wasm_conformance(), 1);
const actual = new Uint8Array(instance.exports.memory.buffer,
  instance.exports.ratio_result_ptr(), instance.exports.ratio_result_len());
assert.deepEqual(JSON.parse(Buffer.from(actual).toString()),
  JSON.parse(fs.readFileSync(process.argv[3], 'utf8')));
assert.equal(instance.exports.ratio_wasm_capacity(10000, 2), 10000);
console.log('Native JSONL and wasm journal bytes, six side planes, four prefix digests, and figures agree; both views retain 10,000 lots.');
