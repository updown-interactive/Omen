import {mountOmen} from './ui/host.mjs';
const errorBox=document.getElementById('omen-error');
const report=error=>{errorBox.hidden=false;errorBox.textContent=error.message??String(error);console.error(error);};
const pendingPermissions=new Map();let next=1;
const postNative=window.OmenNative?.postMessage?message=>window.OmenNative.postMessage(JSON.stringify(message)):window.webkit?.messageHandlers?.omenNative?message=>window.webkit.messageHandlers.omenNative.postMessage(message):null;
window.OmenReady=(async()=>{
try {
  const config=await(await fetch('./app.json')).json();
  const host=await mountOmen({root:document.getElementById('omen-root'),wasmURL:'./app.wasm',allowlist:config.permissions??[],onError:report,
    nativeSend:postNative?bytes=>postNative({kind:'plugin',frame:JSON.parse(new TextDecoder().decode(bytes))}):null,
    requestNativePermission:postNative?permission=>new Promise(resolve=>{const id=String(next++);pendingPermissions.set(id,resolve);postNative({kind:'permission',id,permission});}):null,
  });
  host.permissionResult=(id,status)=>{pendingPermissions.get(String(id))?.(status);pendingPermissions.delete(String(id));};
  window.OmenHost=host;
  window.addEventListener('beforeunload',()=>{pendingPermissions.forEach(resolve=>resolve('denied'));pendingPermissions.clear();host.dispose();});
  document.title=config.name??'Omen';
}catch(error){report(error);throw error;}
})();
