// Append-only protobuf object frames on an already exclusively owned handle.
// Ownership is held by worker.mjs; create:true alone is never a claim.
import {frameKey, compareUtf8, utf8} from './wire.mjs';
export const MAX_FRAME = 16 * 1024 * 1024;
const U64_MAX = (1n << 64n) - 1n;
function sequence(value) {
  if (!/^\+?[0-9]+$/.test(value)) return null;
  const normalized=value.replace(/^\+/, '').replace(/^0+/, '') || '0';
  if (normalized.length > 20) return null;
  const n=BigInt(normalized); return n <= U64_MAX ? n : null;
}
export class ObjectLog {
  constructor(handle, validate) {
    this.handle=handle; this.validate=validate; this.index=new Map(); this.heights=new Map();
    this.end=0; this.poisoned=false; this.recoveredTailBytes=0;
    const size=handle.getSize();
    if (!Number.isSafeInteger(size) || size < 0) throw new Error('unsupported OPFS file size');
    while(this.end < size) {
      const remaining=size-this.end;
      if(remaining<4) { this.recover(size); break; }
      const length=new DataView(this.read(this.end,4).buffer).getUint32(0,true);
      if(!length || length>MAX_FRAME) throw new Error('unsupported or corrupt OPFS frame length');
      if(remaining<length+8) { this.recover(size); break; }
      const frame=this.read(this.end+4,length);
      const trailer=new DataView(this.read(this.end+4+length,4).buffer).getUint32(0,true);
      if(trailer!==((~length)>>>0) || !validate(frame)) throw new Error('complete OPFS frame is corrupt');
      const key=frameKey(frame);
      if(this.index.has(key)) throw new Error('duplicate immutable OPFS object');
      this.index.set(key,{at:this.end+4,length}); this.noteHeight(key); this.end+=length+8;
    }
  }
  live() { if(this.poisoned) throw new Error('OPFS owner requires recovery'); }
  recover(size) {
    // Only a trailing incomplete record can be discarded, after exclusive
    // ownership. A complete corrupt record never becomes an empty book.
    this.handle.truncate(this.end); this.handle.flush(); this.recoveredTailBytes=size-this.end;
  }
  read(at,length) {
    const bytes=new Uint8Array(length); let got=0;
    while(got<length) {
      const n=this.handle.read(bytes.subarray(got),{at:at+got});
      if(!Number.isInteger(n) || n<=0 || n>length-got) throw new Error('short OPFS read');
      got+=n;
    }
    return bytes;
  }
  noteHeight(key) {
    const split=key.lastIndexOf('/'), prefix=key.slice(0,split+1), n=sequence(key.slice(split+1));
    if(n!==null && n>(this.heights.get(prefix)||0n)) this.heights.set(prefix,n);
  }
  put(frame) {
    this.live();
    if(!frame.length || frame.length>MAX_FRAME || !this.validate(frame)) throw new Error('invalid OPFS object frame');
    const key=frameKey(frame); if(this.index.has(key)) return false;
    const record=new Uint8Array(frame.length+8), view=new DataView(record.buffer);
    view.setUint32(0,frame.length,true); record.set(frame,4); view.setUint32(frame.length+4,(~frame.length)>>>0,true);
    if(!Number.isSafeInteger(this.end+record.length)) throw new Error('OPFS file address overflow');
    try {
      let written=0;
      while(written<record.length) {
        const n=this.handle.write(record.subarray(written),{at:this.end+written});
        if(!Number.isInteger(n) || n<=0 || n>record.length-written) throw new Error('short OPFS write');
        written+=n;
      }
      this.handle.flush();
      this.index.set(key,{at:this.end+4,length:frame.length}); this.noteHeight(key);
      this.end+=record.length;
      return true;
    } catch(error) { this.poisoned=true; throw error; }
  }
  get(key) { this.live(); const record=this.index.get(key); return record ? this.read(record.at,record.length) : null; }
  keys(prefix) {
    this.live(); const keys=[]; let bytes=0;
    for(const key of this.index.keys()) if(key.startsWith(prefix)) {
      bytes+=utf8(key).length+12;
      if(bytes>MAX_FRAME) throw new Error('OPFS key listing exceeds the response limit');
      keys.push(key);
    }
    return keys.sort(compareUtf8);
  }
  height(prefix) {
    this.live();
    if(prefix.endsWith('/')) return this.heights.get(prefix)||0n;
    let maximum=0n;
    for(const key of this.index.keys()) if(key.startsWith(prefix)) {
      const n=sequence(key.slice(prefix.length)); if(n!==null && n>maximum) maximum=n;
    }
    return maximum;
  }
}
