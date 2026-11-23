import test from 'node:test';
import assert from 'node:assert/strict';
import { createNativeHost, sendMessage } from './host.mjs';
const encode = value => new TextEncoder().encode(JSON.stringify(value));
const settle = () => new Promise(resolve => setImmediate(resolve));
const request = (id, plugin, operation, payload) => ({ type:'call', api_version:1, id, plugin, operation, payload });
function filesystem() {
  const files = new Map();
  return { navigator: { storage: { async getDirectory() { return { async getFileHandle(name, options) {
    if (!files.has(name) && !options?.create) throw new Error('Missing file');
    return { async getFile() { const bytes = new TextEncoder().encode(files.get(name) ?? ''); return { size:bytes.length, async arrayBuffer() { return bytes.buffer; } }; },
      async createWritable() { let pending = ''; return { async write(value) {pending = value;}, async close() {files.set(name,pending);}, async abort() {} }; } };
  } }; } } } };
}
test('denies ungranted access, round trips origin files, emits events and rejects traversal', async () => {
  const messages=[];const host=createNativeHost({ context:101,allowlist:['fs.read','fs.write'],receive:bytes=>messages.push(JSON.parse(new TextDecoder().decode(bytes))),environment:filesystem() });
  sendMessage(101,encode(request('1','omen.filesystem','write_text',{path:'note',contents:'Hello'})));await settle();assert.equal(messages.pop().error.code,'permission_denied');
  assert.equal(await host.requestPermission('fs.write'),'granted');assert.equal(await host.requestPermission('fs.read'),'granted');
  host.dispatch(encode(request('2','omen.filesystem','write_text',{path:'note',contents:'Hello'})));await settle();assert.equal(messages[0].topic,'changed');assert.equal(messages[1].payload,null);messages.length=0;
  host.dispatch(encode(request('3','omen.filesystem','read_text',{path:'note'})));await settle();assert.equal(messages.pop().payload,'Hello');
  host.dispatch(encode(request('4','omen.filesystem','read_text',{path:'../secret'})));await settle();assert.equal(messages.pop().error.code,'invalid_request');
  assert.equal(await host.requestPermission('camera'),'denied');host.close();assert.equal(sendMessage(101,encode(request('5','omen.filesystem','read_text',{path:'note'}))),false);
});
test('cancellation suppresses stale host responses and version checks reject malformed messages', async () => {
  const messages=[];const host=createNativeHost({context:102,allowlist:['fs.write'],receive:bytes=>messages.push(bytes),environment:filesystem()});await host.requestPermission('fs.write');
  assert.equal(host.dispatch(encode({...request('1','omen.filesystem','write_text',{path:'x',contents:'x'}),api_version:99})),false);
  host.dispatch(encode(request('2','omen.filesystem','write_text',{path:'x',contents:'x'})));host.dispatch(encode({type:'cancel',api_version:1,id:'2'}));await settle();assert.equal(messages.length,0);host.close();
});
test('notifications require both allowlist and current browser grant', async () => {
  const shown=[];class Notification {static permission='default';static async requestPermission(){this.permission='granted';return 'granted';}constructor(title,{body}){shown.push([title,body]);}}
  const messages=[];const host=createNativeHost({context:103,allowlist:['notifications'],receive:bytes=>messages.push(JSON.parse(new TextDecoder().decode(bytes))),environment:{Notification}});
  await host.requestPermission('notifications');host.dispatch(encode(request('1','omen.notifications','show',{title:'Title',body:'Body'})));await settle();assert.deepEqual(shown,[['Title','Body']]);
  Notification.permission='denied';host.dispatch(encode(request('2','omen.notifications','show',{title:'Title',body:'Body'})));await settle();assert.equal(messages.pop().error.code,'permission_denied');host.close();
});
test('camera releases acquired tracks when capture fails and revoked grants prevent access', async () => {
  let stopped=0;let opened=0;
  const environment={navigator:{mediaDevices:{async getUserMedia(){opened++;return{getTracks(){return[{stop(){stopped++;}}];}};}}},document:{createElement(name){if(name==='video')return{async play(){throw new Error('No video frame');},pause(){},srcObject:null};throw new Error('Unexpected canvas');}}};
  const messages=[];const host=createNativeHost({context:104,allowlist:['camera'],receive:bytes=>messages.push(JSON.parse(new TextDecoder().decode(bytes))),environment});
  assert.equal(await host.requestPermission('camera'),'granted');assert.equal(stopped,1);
  host.dispatch(encode(request('1','omen.camera','capture',{width:320,height:240})));await settle();assert.equal(messages.pop().error.code,'native');assert.equal(stopped,2);
  host.revoke('camera');host.dispatch(encode(request('2','omen.camera','capture',{width:320,height:240})));await settle();assert.equal(messages.pop().error.code,'permission_denied');assert.equal(opened,2);host.close();
});
test('storage permission probes availability and stays denied when an exposed API rejects', async () => {
  let available=false;
  const environment={navigator:{storage:{async getDirectory(){if(!available)throw new Error('Storage unavailable');return {};}}}};
  const host=createNativeHost({context:105,allowlist:['fs.write'],receive:()=>{},environment});
  assert.equal(await host.requestPermission('fs.write'),'denied');
  available=true;assert.equal(await host.requestPermission('fs.write'),'granted');
  host.close();
});
