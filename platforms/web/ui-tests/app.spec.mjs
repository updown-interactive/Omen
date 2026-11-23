import {test,expect} from '@playwright/test';
test('real WASM state, native controls, async completion, and modal focus',async({page})=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto('/');await page.bringToFront();await expect(page.getByText('Count: 0',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Increment',exact:true}).click();await expect(page.getByText('Count: 1',{exact:true})).toBeVisible();
  await page.getByRole('textbox',{name:'Your name'}).fill('Ada 👩‍💻');await expect(page.getByText('Hello, Ada 👩‍💻',{exact:true})).toBeVisible();
  const field=page.getByRole('textbox',{name:'Your name'});await field.evaluate(element=>{element.focus();element.setSelectionRange(2,2);});
  await page.getByRole('button',{name:'Increment',exact:true}).click();await expect(field).toHaveValue('Ada 👩‍💻');
  await page.getByRole('switch',{name:'Enabled',exact:true}).uncheck();await expect(page.getByText('Disabled',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Async action',exact:true}).click();await expect(page.getByText('Complete',{exact:true})).toBeVisible();
  const open=page.getByRole('button',{name:'Open dialog',exact:true});await open.click();await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).toBeHidden();await expect(open).toBeFocused();
  await page.getByRole('button',{name:'Native file round trip',exact:true}).click();const writable=await page.evaluate(async()=>{try{await navigator.storage.getDirectory();return typeof globalThis.FileSystemFileHandle?.prototype.createWritable==='function'?'yes':'unsupported';}catch{return 'denied';}});
  await expect(page.getByText(writable==='yes'?'Native file: Hello from Rust':writable==='denied'?'PermissionDenied: Native permission denied':'Unsupported: Browser does not support filesystem writes',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Toggle theme',exact:true}).click();await expect(page.getByText('Count: 2',{exact:true})).toBeVisible();
  await page.screenshot({path:test.info().outputPath('platform-ui.png'),fullPage:true});expect(errors).toEqual([]);
});
test('composition is not overwritten by a state update, and background state persists',async({page})=>{
  await page.goto('/');await page.bringToFront();const field=page.getByRole('textbox',{name:'Your name'});await expect(field).toBeVisible();
  await field.evaluate(element=>{element.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true,data:''}));element.value='に';element.dispatchEvent(new InputEvent('input',{bubbles:true,data:'に',isComposing:true}));});
  await page.getByRole('button',{name:'Increment',exact:true}).click();await expect(field).toHaveValue('に');
  await field.evaluate(element=>{element.value='日本';element.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'日本'}));});
  await expect(page.getByText('Hello, 日本',{exact:true})).toBeVisible();
  await page.evaluate(()=>window.OmenHost.lifecycle('background'));await page.evaluate(()=>window.OmenHost.lifecycle('foreground'));
  await expect(page.getByText('Count: 1',{exact:true})).toBeVisible();await expect(field).toHaveValue('日本');
});
test('long press is cancelled by pointer movement and swipe uses logical pixels',async({page,isMobile})=>{
  await page.goto('/');await page.bringToFront();const area=page.getByText('Hold or swipe left',{exact:true});await expect(area).toBeVisible();
  await area.dispatchEvent('pointerdown',{pointerId:1,button:0,clientX:120,clientY:20});
  await area.dispatchEvent('pointermove',{pointerId:1,clientX:90,clientY:20});
  await area.dispatchEvent('pointercancel',{pointerId:1});
  await page.waitForTimeout(550);await expect(page.getByText('Count: 0',{exact:true})).toBeVisible();
  await area.dispatchEvent('pointerdown',{pointerId:2,button:0,clientX:120,clientY:20});
  await area.dispatchEvent('pointerup',{pointerId:2,button:0,clientX:30,clientY:20});await expect(page.getByText('Count: 100',{exact:true})).toBeVisible();
  await area.dispatchEvent('pointerdown',{pointerId:3,button:0,clientX:120,clientY:20});
  await expect(page.getByText('Count: 110',{exact:true})).toBeVisible();await area.dispatchEvent('pointerup',{pointerId:3,button:0,clientX:120,clientY:20});
});
