import {createRenderer} from './renderer.mjs';
import {createNativeHost} from '../host.mjs';
const encoder=new TextEncoder();const decoder=new TextDecoder();
/** Start exactly one packaged Omen WASM instance in a native browser root. */
export async function mountOmen({root,wasmURL,wasmBytes,allowlist=[],nativeSend,requestNativePermission,onError=console.error}) {
  let nativeMetrics;let instance;let closed=false;let foreground=true;let pending=false;const timers=new Map();const cleanups=[];
  const dark=matchMedia('(prefers-color-scheme: dark)');const reduced=matchMedia('(prefers-reduced-motion: reduce)');
  const fine=matchMedia('(any-pointer: fine)');const coarse=matchMedia('(any-pointer: coarse)');
  const send=event=>queueMicrotask(()=>{
    if(closed||!instance)return;
    const bytes=encoder.encode(JSON.stringify(event));const pointer=instance.exports.omen_web_alloc(bytes.length);
    if(!pointer){onError(new Error('Host event exceeds Omen input limit'));return;}
    new Uint8Array(instance.exports.memory.buffer,pointer,bytes.length).set(bytes);
    if(instance.exports.omen_web_event(pointer,bytes.length)!==0)onError(new Error('Omen rejected a host event'));
  });
  const renderer=createRenderer(root,send,{reducedMotion:()=>reduced.matches});
  const native=createNativeHost({context:1,allowlist,receive:bytes=>send({type:'native',message:JSON.parse(decoder.decode(bytes))})});
  const schedule=()=>{if(closed||!foreground||pending)return;pending=true;requestAnimationFrame(()=>{pending=false;if(!closed&&foreground&&instance?.exports.omen_web_render()!==0)onError(new Error('Omen rendering failed'));});};
  const imports={omen:{
    scene(pointer,len){const bytes=new Uint8Array(instance.exports.memory.buffer,pointer,len);const scene=JSON.parse(decoder.decode(bytes));renderer.patch(scene);},
    schedule,
    timer_start(id,milliseconds){timers.set(id,setTimeout(()=>{timers.delete(id);send({type:'timer',id});},milliseconds));},
    timer_cancel(id){clearTimeout(timers.get(id));timers.delete(id);},
    permission_start(id,pointer,len){const permission=decoder.decode(new Uint8Array(instance.exports.memory.buffer,pointer,len));Promise.resolve().then(()=>requestNativePermission?requestNativePermission(permission):native.requestPermission(permission)).then(status=>send({type:'permission',id,status})).catch(()=>send({type:'permission',id,status:'denied'}));},
    native_send(pointer,len){const bytes=new Uint8Array(instance.exports.memory.buffer,pointer,len).slice();if(nativeSend){queueMicrotask(()=>nativeSend(bytes));return 1;}return native.dispatch(bytes)?1:0;},
  }};
  try {
    const bytes=wasmBytes??await (await fetch(wasmURL)).arrayBuffer();
    const loaded=await WebAssembly.instantiate(bytes,imports);instance=loaded.instance??loaded;
    if(instance.exports.omen_app_start()!==0)throw new Error('Omen startup failed');
  }catch(error){renderer.dispose();native.close();throw error;}
  const listen=(target,name,fn)=>{target.addEventListener(name,fn);cleanups.push(()=>target.removeEventListener(name,fn));};
  const probe=document.createElement('div');Object.assign(probe.style,{position:'fixed',visibility:'hidden',fontSize:'medium',width:'1em',padding:'env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)',pointerEvents:'none'});document.body.append(probe);
  const viewport=()=>{
    const style=getComputedStyle(probe);const visual=window.visualViewport;
    const height=Math.min(visual?.height??innerHeight,nativeMetrics?.height??Infinity);
    const focused=document.activeElement;
    const typing=focused?.matches('input, textarea, [contenteditable="true"]');
    const keyboardBottom=typing&&(visual?.scale??1)===1?Math.max(0,innerHeight-height):0;
    root.style.setProperty('--omen-visible-height',`${height}px`);
    send({
      type:'viewport',width:root.clientWidth||innerWidth,height,
      insets:nativeMetrics?.insets??[style.paddingTop,style.paddingRight,style.paddingBottom,style.paddingLeft].map(value=>parseFloat(value)||0),
      system_dark:dark.matches,reduced_motion:reduced.matches,
      pixel_ratio:window.devicePixelRatio||1,
      touch:navigator.maxTouchPoints>0||coarse.matches,mouse:fine.matches,
      keyboard_bottom:keyboardBottom,text_scale:nativeMetrics?.text_scale??(parseFloat(style.fontSize)/16||1),
    });
  };
  const lifecycle=state=>{foreground=state==='foreground';send({type:'lifecycle',state});if(foreground){viewport();schedule();}};
  listen(window,'resize',viewport);if(window.visualViewport){listen(window.visualViewport,'resize',viewport);listen(window.visualViewport,'scroll',viewport);}
  listen(dark,'change',viewport);listen(reduced,'change',viewport);
  listen(fine,'change',viewport);listen(coarse,'change',viewport);
  listen(root,'focusin',viewport);listen(root,'focusout',()=>queueMicrotask(viewport));
  if(typeof ResizeObserver==='function'){
    let width=root.clientWidth;let textSize=probe.offsetWidth;
    const observer=new ResizeObserver(()=>{
      if(root.clientWidth!==width||probe.offsetWidth!==textSize){width=root.clientWidth;textSize=probe.offsetWidth;viewport();}
    });
    observer.observe(root);observer.observe(probe);cleanups.push(()=>observer.disconnect());
  }
  let resolution;
  const trackResolution=()=>{
    resolution?.removeEventListener('change',onResolution);
    resolution=matchMedia(`(resolution: ${window.devicePixelRatio||1}dppx)`);
    resolution.addEventListener('change',onResolution);
  };
  const onResolution=()=>{trackResolution();viewport();};
  trackResolution();cleanups.push(()=>resolution?.removeEventListener('change',onResolution));
  listen(document,'visibilitychange',()=>lifecycle(document.hidden?'background':'foreground'));
  listen(window,'pagehide',()=>lifecycle('background'));listen(window,'pageshow',()=>lifecycle('foreground'));
  const host={
    lifecycle,
    back:()=>renderer.back(),
    setNativeMetrics(metrics){nativeMetrics=metrics;viewport();},
    requestPermission:permission=>requestNativePermission?requestNativePermission(permission):native.requestPermission(permission),
    receiveNative:message=>send({type:'native',message:typeof message==='string'?JSON.parse(message):message}),
    dispose(){if(closed)return;instance.exports.omen_web_dispose();closed=true;timers.forEach(clearTimeout);timers.clear();cleanups.forEach(cleanup=>cleanup());probe.remove();renderer.dispose();native.close();},
  };
  viewport();if(document.hidden)lifecycle('background');else schedule();return host;
}
