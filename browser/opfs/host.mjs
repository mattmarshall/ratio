import {MAX_FRAME} from './object-log.mjs';
import {text,encodeKeys} from './wire.mjs';
function refused(error, code) {
  if(error instanceof WebAssembly.RuntimeError) throw error;
  return code;
}
export function imports(memory, bindings) {
  const replies=new Map();
  const owned=id=>{
    const entry=bindings.get(id);
    if(!entry || !entry.active() || entry.log.poisoned) throw new Error('OPFS binding has no exclusive owner');
    return entry.log;
  };
  const bytes=(pointer,length)=>{
    pointer>>>=0; length>>>=0;
    if(length>MAX_FRAME || pointer+length>memory().buffer.byteLength) throw new Error('invalid wasm buffer');
    return new Uint8Array(memory().buffer,pointer,length);
  };
  return {
    ratio_opfs_owner:id=>{try {owned(id);return 1;}catch(error){return refused(error,0);}},
    ratio_opfs_put:(id,p,n)=>{try{return owned(id).put(bytes(p,n).slice()) ? 1 : 0;}catch(error){return refused(error,-1);}},
    ratio_opfs_get:(id,p,n)=>{try{
      const result=owned(id).get(text(bytes(p,n))); if(result===null){replies.delete(id);return -1;}
      replies.set(id,result);return result.length;
    }catch(error){replies.delete(id);return refused(error,-2);}},
    ratio_opfs_list:(id,p,n)=>{try{
      const result=encodeKeys(owned(id).keys(text(bytes(p,n))));
      if(result.length>MAX_FRAME) throw new Error('OPFS listing too large');
      replies.set(id,result);return result.length;
    }catch(error){replies.delete(id);return refused(error,-2);}},
    ratio_opfs_read:(id,p,n)=>{try{
      owned(id); const result=replies.get(id);
      if(!result || result.length!==(n>>>0)) throw new Error('OPFS reply length changed');
      bytes(p,n).set(result); replies.delete(id);return result.length;
    }catch(error){replies.delete(id);return refused(error,-2);}},
    ratio_opfs_height:(id,p,n,out)=>{try{
      const height=owned(id).height(text(bytes(p,n)));
      new DataView(memory().buffer).setBigUint64(out>>>0,height,true);return 1;
    }catch(error){return refused(error,0);}},
    ratio_monotonic_nanos:()=>BigInt(Math.floor(performance.now()*1000000)),
  };
}
export function validate(instance,frame) {
  const p=instance.exports.ratio_opfs_alloc(frame.length)>>>0;
  try {
    new Uint8Array(instance.exports.memory.buffer,p,frame.length).set(frame);
    return instance.exports.ratio_opfs_validate(p,frame.length)===1;
  } finally {instance.exports.ratio_opfs_free(p,frame.length);}
}
export function probe(instance,request) {
  const p=instance.exports.ratio_opfs_alloc(request.length)>>>0;
  try {
    new Uint8Array(instance.exports.memory.buffer,p,request.length).set(request);
    instance.exports.ratio_opfs_probe(p,request.length);
    return new Uint8Array(instance.exports.memory.buffer,
      instance.exports.ratio_opfs_result_ptr()>>>0,instance.exports.ratio_opfs_result_len()>>>0).slice();
  } finally {instance.exports.ratio_opfs_free(p,request.length);}
}
