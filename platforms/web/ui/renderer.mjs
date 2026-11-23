// Stable DOM reconciliation. Scene text never enters innerHTML or executable markup.
export function createRenderer(root, send, { document = root.ownerDocument, reducedMotion = () => false } = {}) {
  const records = new Map();
  const composing = new WeakSet();
  const gestures = new Map();
  const removers = [];
  let closed = false;
  let dialogs = [];
  const listen = (target, name, fn, options) => { target.addEventListener(name, fn, options); removers.push(() => target.removeEventListener(name, fn, options)); };
  const idFor = target => target?.closest?.('[data-omen-id]')?.dataset.omenId;
  const recordFor = target => records.get(idFor(target));
  const emit = message => { if (!closed) send(message); };
  const selector = 'button:not([disabled]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),a[href],[tabindex="0"]';
  function focusable(panel) { return [...panel.querySelectorAll(selector)].filter(node => !node.closest('[inert]')); }
  function closeRecord(record) {
    record.animation?.cancel();
    if (record.panel) {
      if (record.panel.open && record.panel.close) record.panel.close();
      if (record.previousFocus?.isConnected && !record.previousFocus.closest('[inert]')) record.previousFocus.focus({ preventScroll: true });
    }
    record.element.remove();
  }
  function construct(node) {
    const presentation = node.attributes['data-presentation'];
    let element = document.createElement(node.tag);
    let control = element;
    let slot = element;
    let textLabel;
    if (node.attributes['data-visible-label']) {
      element = document.createElement('label');
      element.className = 'omen-check';
      textLabel = document.createElement('span');
      element.append(control, textLabel); slot = null;
    }
    const record = { id: node.id, element, control, slot, attributes: {}, css: {}, tag: node.tag, node, textLabel };
    element.dataset.omenId = node.id; control.dataset.omenId = node.id;
    if (presentation) {
      record.base = document.createElement('div');
      record.panel = document.createElement(presentation === 'popover' ? 'div' : 'dialog');
      record.panel.className = `omen-presentation omen-${presentation}`;
      record.panel.setAttribute('role', presentation === 'popover' ? 'group' : 'dialog');
      record.panel.tabIndex = -1;
      if (presentation !== 'popover') record.panel.setAttribute('aria-modal', 'true');
      record.panel.dataset.dismissId = node.id;
      element.append(record.base, record.panel);
      record.slot = null;
      listen(record.panel, 'cancel', event => { event.preventDefault(); emit({ type: 'dismiss', id: node.id }); });
      listen(record.panel, 'click', event => { if (event.target === record.panel && presentation !== 'popover') emit({ type: 'dismiss', id: node.id }); });
    }
    return record;
  }
  function patchChildren(parent, children, seen) {
    const elements = children.map(child => patch(child, seen).element);
    let cursor = parent.firstChild;
    for (const element of elements) {
      if (element === cursor) cursor = cursor.nextSibling;
      else if (element.isConnected && parent.moveBefore) parent.moveBefore(element,cursor);
      else parent.insertBefore(element, cursor);
    }
    const desired = new Set(elements);
    for (const child of [...parent.childNodes]) if (!desired.has(child)) parent.removeChild(child);
  }
  function patch(node, seen) {
    if (seen.has(node.id)) throw new Error('Duplicate scene identity');
    seen.add(node.id);
    let record = records.get(node.id);
    const fresh = !record || record.tag !== node.tag || !!record.panel !== !!node.attributes['data-presentation'];
    if (fresh) { if (record) closeRecord(record); record = construct(node); records.set(node.id, record); }
    record.node = node;
    const { element, control } = record;
    for (const key of Object.keys(record.attributes)) if (!(key in node.attributes)) control.removeAttribute(key);
    for (const [key, value] of Object.entries(node.attributes)) {
      if (key === 'data-visible-label') { record.textLabel.textContent = value; continue; }
      if (record.attributes[key] !== value) control.setAttribute(key, value);
    }
    record.attributes = node.attributes;
    for (const key of Object.keys(record.css)) if (!(key in node.style)) element.style.removeProperty(key);
    for (const [key, value] of Object.entries(node.style)) if (record.css[key] !== value) element.style.setProperty(key, value);
    record.css = node.style;
    if(node.gestures.some(gesture=>gesture==='swipe_left'||gesture==='swipe_right'))element.style.touchAction='pan-y';
    else if(node.gestures.some(gesture=>gesture==='swipe_up'||gesture==='swipe_down'))element.style.touchAction='pan-x';
    else element.style.removeProperty('touch-action');
    if (record.textLabel) { element.style.flexDirection = 'row'; element.style.alignItems = 'center'; element.style.gap = '8px'; }
    if (!node.children.length && !record.panel && !record.textLabel && !['INPUT','IMG','HR','PROGRESS'].includes(control.tagName) && control.textContent !== node.text) control.textContent = node.text;
    if (node.value != null && !composing.has(control) && control.value !== node.value) {
      const active = document.activeElement === control;
      const start = control.selectionStart; const end = control.selectionEnd;
      control.value = node.value;
      if (active && start != null && end != null) control.setSelectionRange(Math.min(start,node.value.length), Math.min(end,node.value.length));
    }
    if (node.checked != null) control.checked = node.checked;
    if (record.panel) {
      patchChildren(record.base, node.children.slice(0,1), seen);
      patchChildren(record.panel, node.children.slice(1), seen);
      const blocking = node.attributes['data-presentation'] !== 'popover';
      const active=node.children.length>1;
      record.base.inert = blocking&&active;
      if(!active){
        if(record.active){if(record.panel.open&&record.panel.close)record.panel.close();record.restoreAfterPatch=true;}
        record.panel.hidden=true;
      }else{
        record.panel.hidden=false;
        if(!record.active){record.previousFocus=document.activeElement;record.openAfterPatch=true;}
      }
      record.active=active;
    } else if (record.slot && node.children.length) patchChildren(record.slot,node.children,seen);
    else if (record.slot && record.previousChildren) patchChildren(record.slot,[],seen);
    record.previousChildren = !!node.children.length;
    const target = `${node.motion?.opacity ?? node.style.opacity ?? 1}/${node.motion?.offset ?? node.style.transform}`;
    if (node.motion && !reducedMotion() && (fresh || record.motionTarget !== target) && element.animate) {
      let fromOpacity = Number.parseFloat(record.lastOpacity ?? node.motion.opacity);
      let fromOffset = record.lastOffset ?? node.motion.offset;
      if (fresh && node.motion.entry) { fromOpacity = 0; fromOffset = [node.motion.offset[0], node.motion.offset[1] + (node.motion.entry === 'slide' ? 16 : 0)]; }
      const keyframes = node.motion.samples.map((sample,index,array) => ({ offset:index/(array.length-1), opacity: Math.max(0,Math.min(1,fromOpacity+(node.motion.opacity-fromOpacity)*sample)), transform:`translate(${fromOffset[0]+(node.motion.offset[0]-fromOffset[0])*sample}px, ${fromOffset[1]+(node.motion.offset[1]-fromOffset[1])*sample}px)` }));
      record.animation?.cancel(); record.animation = element.animate(keyframes,{duration:node.motion.duration_ms,easing:'linear'});
      record.lastOpacity=node.motion.opacity;record.lastOffset=node.motion.offset;
    } else if (!node.motion || reducedMotion()) record.animation?.cancel();
    record.motionTarget=target;
    return record;
  }
  const api = {
    patch(scene) {
      if (closed) return;
      if (scene.version !== 1) throw new Error('Incompatible Omen scene version');
      root.style.background=scene.root.style.background??'#101827';
      root.style.color=scene.root.style.color??'#f8fafc';
      const seen=new Set(); patchChildren(root,[scene.root],seen);
      for (const [id,record] of [...records].reverse()) if (!seen.has(id)) { closeRecord(record); records.delete(id); }
      dialogs=[];
      for (const record of records.values()) if (record.panel) {
        if(record.restoreAfterPatch){record.restoreAfterPatch=false;if(record.previousFocus?.isConnected&&!record.previousFocus.closest('[inert]'))record.previousFocus.focus({preventScroll:true});}
        if(!record.active)continue;
        dialogs.push(record);
        if (record.openAfterPatch) {
          record.openAfterPatch=false;
          if (record.node.attributes['data-presentation']==='popover') { /* Nonblocking surface. */ }
          else if (record.panel.showModal) record.panel.showModal();
          else { record.panel.dataset.fallbackOpen='true'; record.panel.hidden=false; }
          (focusable(record.panel)[0] ?? record.panel).focus({preventScroll:true});
        }
        if (record.node.attributes['data-presentation']==='popover') {
          const anchor=record.base.getBoundingClientRect();
          record.panel.style.position='fixed';record.panel.style.left=`${Math.max(8,Math.min(anchor.left,document.defaultView.innerWidth-record.panel.offsetWidth-8))}px`;
          record.panel.style.top=`${Math.max(8,Math.min(anchor.bottom,document.defaultView.innerHeight-record.panel.offsetHeight-8))}px`;
        }
      }
    },
    back() {
      const top=dialogs.at(-1);if(top){emit({type:'dismiss',id:top.id});return true;}
      const back=[...records.values()].find(record=>record.node.attributes['data-omen-back']);
      if(back){emit({type:'activate',id:back.id});return true;}return false;
    },
    dispose() { if (closed) return; closed=true; gestures.forEach(gesture=>clearTimeout(gesture.timer));gestures.clear();removers.forEach(remove=>remove());[...records.values()].reverse().forEach(closeRecord);records.clear();root.replaceChildren(); },
  };
  listen(root,'compositionstart',event=>composing.add(event.target));
  listen(root,'compositionend',event=>{composing.delete(event.target);const record=recordFor(event.target);if(record?.node.value!=null)emit({type:'input',id:record.id,value:event.target.value});});
  listen(root,'input',event=>{const record=recordFor(event.target);if(record?.node.value!=null && !event.isComposing && !composing.has(event.target))emit({type:'input',id:record.id,value:event.target.value});});
  listen(root,'change',event=>{const record=recordFor(event.target);if(record?.node.checked!=null)emit({type:'check',id:record.id,checked:event.target.checked});});
  listen(root,'click',event=>{
    if (event.target.closest('[data-presentation]')?.dataset.omenId === idFor(event.target)) return;
    let record=recordFor(event.target);
    if(record?.node.value!=null||record?.node.checked!=null)return;
    while(record&&!record.node.actionable&&!record.panel){record=recordFor(record.element.parentElement);}
    if(record?.suppressedClick){record.suppressedClick=false;event.preventDefault();return;}
    if(record?.node.actionable && !record.panel && record.node.checked==null && record.node.value==null){record.control.focus({preventScroll:true});emit({type:'activate',id:record.id});}
  });
  listen(root,'keydown',event=>{
    const top=dialogs.at(-1);
    if(event.key==='Escape'&&top){event.preventDefault();emit({type:'dismiss',id:top.id});return;}
    if(event.key==='Tab'&&top&&top.node.attributes['data-presentation']!=='popover') {
      const nodes=focusable(top.panel);const active=document.activeElement;let index=nodes.indexOf(active);
      if(!nodes.length){event.preventDefault();top.panel.focus();}
      else if(event.shiftKey&&(index<=0)){event.preventDefault();nodes.at(-1).focus();}
      else if(!event.shiftKey&&(index===nodes.length-1||index<0)){event.preventDefault();nodes[0].focus();}
    }
    const record=recordFor(event.target);
    if(record?.node.actionable&&record.control.tagName!=='BUTTON'&&['Enter',' '].includes(event.key)&&record.node.value==null&&record.node.checked==null&&!record.panel){event.preventDefault();emit({type:'activate',id:record.id});}
  });
  listen(root,'focusin',event=>{if(event.target.matches('input,textarea'))queueMicrotask(()=>event.target.scrollIntoView?.({block:'nearest',inline:'nearest'}));});
  listen(root,'contextmenu',event=>{if(recordFor(event.target)?.node.gestures.includes('long_press'))event.preventDefault();});
  listen(root,'pointerdown',event=>{
    const record=recordFor(event.target);if(!record?.node.gestures.length||event.button!==0)return;
    record.suppressedClick=false;
    const gesture={record,x:event.clientX,y:event.clientY,time:performance.now(),long:false};
    if(record.node.gestures.includes('long_press'))gesture.timer=setTimeout(()=>{gesture.long=true;record.suppressedClick=true;emit({type:'gesture',id:record.id,gesture:'long_press'});},500);
    gestures.set(event.pointerId,gesture);
  });
  listen(root,'pointermove',event=>{const gesture=gestures.get(event.pointerId);if(gesture&&Math.hypot(event.clientX-gesture.x,event.clientY-gesture.y)>10)clearTimeout(gesture.timer);});
  const cancel=event=>{const gesture=gestures.get(event.pointerId);if(gesture)clearTimeout(gesture.timer);gestures.delete(event.pointerId);};
  listen(root,'pointercancel',cancel);
  listen(root,'pointerup',event=>{
    const gesture=gestures.get(event.pointerId);cancel(event);if(!gesture||gesture.long||performance.now()-gesture.time>1200)return;
    const dx=event.clientX-gesture.x,dy=event.clientY-gesture.y;if(Math.max(Math.abs(dx),Math.abs(dy))<40)return;
    const direction=Math.abs(dx)>Math.abs(dy)?(dx<0?'left':'right'):(dy<0?'up':'down');
    if(gesture.record.node.gestures.includes(`swipe_${direction}`)){gesture.record.suppressedClick=true;emit({type:'gesture',id:gesture.record.id,gesture:`swipe_${direction}`});}
  });
  return api;
}
