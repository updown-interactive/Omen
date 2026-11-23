// Origin-scoped native host. This module is also imported by omen-plugin-web's wasm bindings.
const registryKey = Symbol.for("dev.omen.nativeHosts.v1");
const hosts = globalThis[registryKey] ??= new Map();
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const LIMIT = 1_048_576;
class Failure extends Error { constructor(code, message) { super(message); this.code = code; } }
const check = (condition, message = 'Invalid native request') => { if (!condition) throw new Failure('invalid_request', message); };

/** Install a reviewed native host for one Rust/WASM context; receive accepts Uint8Array. */
export function createNativeHost({ context, allowlist = [], receive, environment = globalThis }) {
  if (hosts.has(context)) throw new Error('Duplicate native host context');
  const allowed = new Set(allowlist);
  const granted = new Set();
  const calls = new Map();
  let closed = false;
  const api = {
    capabilities() {
      return { filesystem: environment.isSecureContext !== false && !!environment.navigator?.storage?.getDirectory, notifications: environment.isSecureContext !== false && !!environment.Notification, camera: environment.isSecureContext !== false && !!environment.navigator?.mediaDevices?.getUserMedia };
    },
    /** Must run from a user gesture for APIs requiring transient activation. */
    async requestPermission(permission) {
      if (closed || !allowed.has(permission)) return 'denied';
      if (permission === 'fs.read' || permission === 'fs.write') {
        if (!api.capabilities().filesystem) return 'denied';
        // Exposed APIs can still reject storage in private contexts or under quota pressure.
        try { await environment.navigator.storage.getDirectory(); } catch { granted.delete(permission); return 'denied'; }
        if (closed) return 'denied'; granted.add(permission); return 'granted';
      }
      if (permission === 'notifications' && api.capabilities().notifications) {
        let result; try { result = await environment.Notification.requestPermission(); } catch { granted.delete(permission); return 'denied'; }
        if (closed) return 'denied';
        if (result === 'granted') granted.add(permission); else granted.delete(permission);
        return result === 'granted' ? 'granted' : 'denied';
      }
      if (permission === 'camera' && api.capabilities().camera) {
        try { const stream = await environment.navigator.mediaDevices.getUserMedia({ video: true, audio: false }); stream.getTracks().forEach(track => track.stop()); if (!closed) granted.add(permission); return closed ? 'denied' : 'granted'; }
        catch { granted.delete(permission); return 'denied'; }
      }
      return 'denied';
    },
    revoke(permission) { granted.delete(permission); },
    close() { if (closed) return; closed = true; calls.forEach(controller => controller.abort()); calls.clear(); granted.clear(); hosts.delete(context); },
    dispatch(bytes) {
      if (closed || bytes.length > LIMIT) return false;
      let message;
      try { message = JSON.parse(decoder.decode(bytes)); } catch { return false; }
      if (message.api_version !== 1 || typeof message.id !== 'string' || !/^[0-9]{1,20}$/.test(message.id)) return false;
      if (message.type === 'cancel') { calls.get(message.id)?.abort(); return true; }
      if (message.type !== 'call' || calls.has(message.id)) return false;
      if (calls.size >= 256) { fail(message.id, new Failure('unavailable', 'Too many native calls')); return true; }
      const controller = new AbortController(); calls.set(message.id, controller);
      Promise.resolve().then(() => invoke(message, controller.signal)).then(payload => {
        if (!controller.signal.aborted && !closed) send({ type: 'response', api_version: 1, id: message.id, payload });
      }).catch(error => { if (!controller.signal.aborted && !closed) fail(message.id, error); })
        .finally(() => calls.delete(message.id));
      return true;
    },
  };
  const send = message => {
    if (closed) return;
    const bytes = encoder.encode(JSON.stringify(message));
    if (bytes.length <= LIMIT) receive(bytes);
    else if (message.id) fail(message.id, new Failure('invalid_request', 'Response exceeds bridge limit'));
  };
  const fail = (id, error) => send({ type: 'error', api_version: 1, id, error: { code: error instanceof Failure ? error.code : 'native', message: error instanceof Failure ? error.message : 'Native operation failed' } });
  async function requirePermission(permission, signal) {
    signal.throwIfAborted();
    if (!allowed.has(permission) || !granted.has(permission)) throw new Failure('permission_denied', 'Native permission not granted');
    if (permission === 'notifications' && environment.Notification?.permission !== 'granted') throw new Failure('permission_denied', 'Notification permission revoked');
    if (permission === 'camera' && environment.navigator?.permissions?.query) {
      try { const status = await environment.navigator.permissions.query({ name: 'camera' }); if (status.state !== 'granted') throw new Failure('permission_denied', 'Camera permission revoked'); }
      catch (error) { if (error instanceof Failure) throw error; /* Browser does not expose camera permission queries. */ }
    }
    signal.throwIfAborted();
  }
  function filename(value) { check(typeof value === 'string' && value.length > 0 && value !== '.' && value !== '..' && !/[\\/\0]/.test(value)); return value; }
  async function invoke({ plugin, operation, payload }, signal) {
    check(payload && typeof payload === 'object' && !Array.isArray(payload));
    switch (`${plugin}/${operation}`) {
      case 'omen.filesystem/read_text': {
        await requirePermission('fs.read', signal);
        const root = await environment.navigator.storage.getDirectory();
        const file = await (await root.getFileHandle(filename(payload.path))).getFile();
        check(file.size <= 262144, 'File exceeds size limit');
        const text = decoder.decode(await file.arrayBuffer()); signal.throwIfAborted(); return text;
      }
      case 'omen.filesystem/write_text': {
        await requirePermission('fs.write', signal); check(typeof payload.contents === 'string' && encoder.encode(payload.contents).length <= 262144);
        const name = filename(payload.path); const root = await environment.navigator.storage.getDirectory();
        signal.throwIfAborted(); const file = await root.getFileHandle(name, { create: true }); if (typeof file.createWritable !== 'function') throw new Failure('unsupported', 'Browser does not support filesystem writes'); const writer = await file.createWritable();
        const abort = () => { void writer.abort().catch(() => {}); }; signal.addEventListener('abort', abort, { once: true });
        try { signal.throwIfAborted(); await writer.write(payload.contents); signal.throwIfAborted(); await writer.close(); }
        catch (error) { await writer.abort().catch(() => {}); throw error; }
        finally { signal.removeEventListener('abort', abort); }
        send({ type: 'event', api_version: 1, plugin, topic: 'changed', payload: { path: name } }); return null;
      }
      case 'omen.notifications/show': {
        await requirePermission('notifications', signal);
        check(typeof payload.title === 'string' && encoder.encode(payload.title).length <= 256 && typeof payload.body === 'string' && encoder.encode(payload.body).length <= 4096);
        new environment.Notification(payload.title, { body: payload.body }); return null;
      }
      case 'omen.camera/capture': {
        await requirePermission('camera', signal);
        check(Number.isInteger(payload.width) && payload.width > 0 && payload.width <= 1920 && Number.isInteger(payload.height) && payload.height > 0 && payload.height <= 1080);
        const stream = await environment.navigator.mediaDevices.getUserMedia({ video: { width: { ideal: payload.width }, height: { ideal: payload.height } }, audio: false });
        const stop = () => stream.getTracks().forEach(track => track.stop()); signal.addEventListener('abort', stop, { once: true });
        const video = environment.document.createElement('video');
        try {
          signal.throwIfAborted(); video.muted = true; video.playsInline = true; video.srcObject = stream; await video.play(); signal.throwIfAborted();
          const canvas = environment.document.createElement('canvas'); canvas.width = Math.min(payload.width, video.videoWidth); canvas.height = Math.min(payload.height, video.videoHeight);
          check(canvas.width > 0 && canvas.height > 0, 'Camera produced no frame');
          canvas.getContext('2d').drawImage(video, 0, 0, canvas.width, canvas.height);
          const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/jpeg', 0.7)); check(blob && blob.size <= 262144, 'Camera image exceeds size limit');
          const data = Array.from(new Uint8Array(await blob.arrayBuffer())); signal.throwIfAborted(); return { mime: 'image/jpeg', data };
        } finally { stop(); video.pause(); video.srcObject = null; signal.removeEventListener('abort', stop); }
      }
      default: throw new Failure('unsupported', 'Native capability unavailable');
    }
  }
  hosts.set(context, api); return api;
}
/** Synchronous WASM transport entry. Native operations complete through receive callbacks. */
export function sendMessage(context, bytes) { return hosts.get(context)?.dispatch(bytes) ?? false; }
