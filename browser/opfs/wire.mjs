// The local probe messages are ratio.storage.v1.OpfsProbeRequest/Reply.
// Journal/configuration bytes remain opaque; no money is decoded here.
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', {fatal: true});
export const utf8 = value => encoder.encode(value);
export const text = bytes => decoder.decode(bytes);
export function concat(parts) {
  const result = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0; for (const part of parts) { result.set(part, at); at += part.length; }
  return result;
}
export function varint(value) {
  let n = BigInt(value); if (n < 0n) throw new Error('negative probe counter');
  const bytes = []; do { const b = Number(n & 127n); n >>= 7n; bytes.push(b | (n ? 128 : 0)); } while (n);
  return Uint8Array.from(bytes);
}
export function readVarint(bytes, cursor) {
  let n = 0n;
  for (let shift = 0n; shift < 70n; shift += 7n) {
    if (cursor.at >= bytes.length) throw new Error('truncated protobuf');
    const b = bytes[cursor.at++]; n |= BigInt(b & 127) << shift;
    if (!(b & 128)) return n;
  }
  throw new Error('oversized protobuf varint');
}
const numeric = (field, value) => concat([varint(field * 8), varint(value)]);
const binary = (field, bytes) => concat([varint(field * 8 + 2), varint(bytes.length), bytes]);
const string = (field, value) => binary(field, utf8(value));
export function fields(bytes) {
  const result = new Map(), cursor = {at: 0};
  while (cursor.at < bytes.length) {
    const tag = Number(readVarint(bytes, cursor)), field = tag >>> 3, kind = tag & 7;
    if (!field) throw new Error('invalid protobuf field');
    let value;
    if (kind === 0) value = readVarint(bytes, cursor);
    else if (kind === 2) {
      const length = Number(readVarint(bytes, cursor));
      if (!Number.isSafeInteger(length) || cursor.at + length > bytes.length) throw new Error('invalid protobuf length');
      value = bytes.slice(cursor.at, cursor.at + length); cursor.at += length;
    } else throw new Error('unsupported probe wire type');
    const values = result.get(field) || []; values.push(value); result.set(field, values);
  }
  return result;
}
const last = (f, n, fallback) => f.get(n)?.at(-1) ?? fallback;
const str = (f, n) => text(last(f, n, new Uint8Array()));
const num = (f, n) => {
  const n64 = last(f, n, 0n); if (n64 > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('oversized probe counter');
  return Number(n64);
};
export function encodeRequest(q) {
  return concat([string(1,q.correlationId),numeric(2,q.operation),string(3,q.namespace || ''),numeric(4,q.binding ?? 1)]);
}
export function decodeRequest(bytes) {
  const f = fields(bytes); return {correlationId:str(f,1), operation:num(f,2), namespace:str(f,3), binding:num(f,4)};
}
export function encodeReply(r) {
  return concat([string(1,r.correlationId || ''),numeric(2,r.passed ? 1 : 0),numeric(3,r.role || 0),
    binary(4,r.fixtureJson || new Uint8Array()),string(5,r.detail || ''),string(6,r.ownerId || ''),
    numeric(7,r.heldLockCount || 0),numeric(8,r.writeAcknowledged ? 1 : 0)]);
}
export function decodeReply(bytes) {
  const f = fields(bytes); return {correlationId:str(f,1),passed:!!num(f,2),role:num(f,3),
    fixtureJson:last(f,4,new Uint8Array()),detail:str(f,5),ownerId:str(f,6),heldLockCount:num(f,7),writeAcknowledged:!!num(f,8)};
}
export function encodeKeys(keys) { return concat(keys.map(key => string(1,key))); }
export function frameKey(bytes) {
  const cursor = {at:0};
  if (readVarint(bytes,cursor) !== 10n) throw new Error('object key must be the first canonical field');
  const length = Number(readVarint(bytes,cursor));
  if (!Number.isSafeInteger(length) || !length || cursor.at + length > bytes.length) throw new Error('invalid object key');
  return text(bytes.subarray(cursor.at,cursor.at + length));
}
export function compareUtf8(a,b) {
  const left=utf8(a), right=utf8(b), n=Math.min(left.length,right.length);
  for(let i=0;i<n;i++) if(left[i]!==right[i]) return left[i]-right[i];
  return left.length-right.length;
}
