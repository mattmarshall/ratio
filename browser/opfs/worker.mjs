// Local conformance shell. The reusable lease and frame log are production
// mechanisms; these commands touch only explicitly named synthetic lab data.
import {ownNamespace} from './lease.mjs';
import {ObjectLog} from './object-log.mjs';
import {imports,validate,probe} from './host.mjs';
import {decodeRequest,decodeReply,encodeReply} from './wire.mjs';
let started=false;
self.onmessage=async event=>{
  let request;
  try {
    request=decodeRequest(new Uint8Array(event.data));
    if(started || request.operation!==1 || !request.correlationId || !request.namespace.startsWith('ratio365lab-'))
      throw new Error('invalid or repeated lab start');
    started=true;
    await ownNamespace(request.namespace,async lease=>{
      const ownerId=crypto.randomUUID(),bindings=new Map();let instance,interrupt=false;
      const handle={
        getSize:()=>lease.handle.getSize(),read:(bytes,options)=>lease.handle.read(bytes,options),
        write:(bytes,options)=>{
          if(interrupt){interrupt=false;
            lease.handle.write(bytes.subarray(0,Math.max(1,Math.floor(bytes.length/2))),options);
            lease.handle.flush();throw new Error('injected unacknowledged partial append');}
          return lease.handle.write(bytes,options);
        },
        truncate:size=>lease.handle.truncate(size),flush:()=>lease.handle.flush(),
      };
      const moduleBytes=await (await fetch(new URL('./opfs_conformance.wasm',import.meta.url))).arrayBuffer();
      instance=(await WebAssembly.instantiate(moduleBytes,{env:imports(()=>instance.exports.memory,bindings)})).instance;
      const log=new ObjectLog(handle,frame=>validate(instance,frame));
      bindings.set(1,{log,active:lease.alive});
      const channel=new BroadcastChannel(`ratio:opfs:probe:${request.namespace}`);
      let release;const lifetime=new Promise(resolve=>{release=resolve;});let queue=Promise.resolve();
      channel.onmessage=event=>{
        const bytes=new Uint8Array(event.data);
        queue=queue.then(async()=>{
          let q;
          try {
            q=decodeRequest(bytes);
            if(!q.correlationId || q.namespace!==request.namespace || q.binding!==1 || q.operation===1)
              throw new Error('foreign or invalid lab command');
            let reply;
            if(q.operation===5) reply={correlationId:q.correlationId,passed:true,
              detail:`exclusive owner; ${log.index.size} immutable objects; ${log.recoveredTailBytes} incomplete tail bytes recovered`};
            else {
              if(q.operation===6) interrupt=true;
              reply=decodeReply(probe(instance,bytes));
            }
            channel.postMessage(encodeReply({...reply,role:1,ownerId,heldLockCount:await lease.count()}));
          } catch(error) {
            // An unexpected trap invalidates this instance. No cached figure
            // is served; release closes the handle before another owner runs.
            bindings.clear();
            channel.postMessage(encodeReply({correlationId:q?.correlationId||'',passed:false,role:3,
              ownerId,detail:`owner discarded: ${String(error)}`}));
            release();
          }
        });
      };
      self.postMessage(encodeReply({correlationId:request.correlationId,passed:true,role:1,ownerId,
        heldLockCount:await lease.count(),detail:`owner ready; recovered ${log.recoveredTailBytes} incomplete tail bytes`}));
      await lifetime;channel.close();self.close();
    },async count=>{
      self.postMessage(encodeReply({correlationId:request.correlationId,passed:true,role:2,
        heldLockCount:count,detail:'client only; another worker owns this backend'}));
      self.close();
    });
  } catch(error) {
    self.postMessage(encodeReply({correlationId:request?.correlationId||'',passed:false,role:3,detail:String(error)}));
    self.close();
  }
};
