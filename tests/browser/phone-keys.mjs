// Model coverage and key editing against a production build with API fixtures.
import assert from 'node:assert/strict';
import { readFileSync, mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });

// Read the catalog's inline desk records to keep all model fixtures on the same source.
// Fail explicitly if the YAML record format changes instead of silently skipping models.
const catalog = readFileSync(fileURLToPath(new URL('../../presets/phones/yealink.yaml', import.meta.url)), 'utf8');
const models = [...catalog.matchAll(/- \{ id: (\w+), name: ([\w-]+), family: desk, accounts: (\d+), line_keys: (\d+)([\s\S]*?)\}\s*\}/g)].map((m) => {
  const field = key => { const match = m[5].match(new RegExp(`${key}: (\\d+|true|false)`)); assert.ok(match, `${m[1]}: ${key}`); return JSON.parse(match[1]); };
  const layout = Object.fromEntries(['width','height','color','touch','left','right','columns','page_key'].map(key => [key, field(key)]));
  layout.pages = JSON.parse(m[5].match(/pages: (\[[^\n]+\])/)[1]);
  return { id:m[1], name:m[2], vendor:'yealink', family:'desk', accounts:Number(m[3]), line_keys:Number(m[4]), line_key_limit:Number(m[3]), layout, speed_dial_keys:true, video:false, ringtone_max_kb:0, wallpaper:false };
});
assert.equal(models.length, 21, 'all desk models must have a fixture');
const base = process.env.BASE || 'http://127.0.0.1:5184';
const screenshots = process.env.SCREENSHOT_DIR;
if (screenshots) mkdirSync(screenshots, { recursive:true });
const account = (index, number, name) => ({account_index:index,device_id:`device-${index}`,extension_id:`extension-${number}`,extension_number:number,display_name:name,phone_label:'',phone_display_name:''});
const extensions = ['101','102','103'].map((number,i) => ({id:`extension-${number}`,number,display_name:['Büro','Empfang','Werkstatt'][i],enabled:true}));
let role = 'admin';
let saved = new Map();
let failSave = false, failResync = false, notified = 1;
const writes=[];
const errors=[];
const context = await browser.newContext({locale:'de-DE',timezoneId:'Europe/Berlin',reducedMotion:'reduce',viewport:{width:1280,height:1000}});
await context.addInitScript(() => {localStorage.setItem('talkops.locale','de');localStorage.setItem('talkops.gettingStarted.hidden','1');});
const phone = id => ({id,mac:'805ec0123456',model:id,name:'Büro',line_keys:[{key:3,type:'blf',value:'102',label:'Empfang',account:1}],last_seen_at:null,last_ip:null,last_firmware:null,ringtone_id:null,wallpaper_id:null,phonebook_sections:['suppliers'],settings:{},accounts:[account(1,'101','Büro'),account(2,'102','Empfang')],...saved.get(id)});
await context.route('**/api/**', async route => {
  const request = route.request(), path = new URL(request.url()).pathname;
  let value=[];
  if(path==='/api/v1/auth/me')value={user:{id:'admin',username:'admin',display_name:'Admin',role,enabled:true},csrf_token:'fixture',permissions:['phones.view']};
  else if(path==='/api/v1/status')value={version:'test',database:{ok:true},freeswitch:{ok:true}};
  else if(path==='/api/v1/phone-models')value=models;
  else if(path==='/api/v1/extensions')value=extensions;
  else if(path==='/api/v1/phone-settings')value={catalog:[],values:{}};
  else if(path==='/api/v1/phonebook-sections')value=[{id:'suppliers',name:'Lieferanten'},{id:'private',name:'Privat'}];
  else if(path==='/api/v1/contacts')value=[
    {id:'c1',name:'Elektro Beispiel',company:'Service',section_id:'suppliers',phone_work:'+49305550101',phone_mobile:'+491705550101',phone_other:''},
    {id:'c2',name:'Globaler Kontakt',company:'',section_id:null,phone_work:'+49895550102',phone_mobile:'',phone_other:''},
    {id:'c3',name:'Privater Kontakt',company:'',section_id:'private',phone_work:'+49405550103',phone_mobile:'',phone_other:''}
  ];
  else if(/^\/api\/v1\/phones\/[^/]+\/resync$/.test(path)) {
    writes.push({type:'resync'});
    return route.fulfill({status:failResync?503:200,json:failResync?{error:'unavailable',message:'Offline fixture'}:{notified}});
  } else if(/^\/api\/v1\/phones\/[^/]+$/.test(path)) {
    const id=path.split('/').at(-1);
    if(request.method()==='PUT') {
      writes.push({type:'save',input:request.postDataJSON()});
      if(failSave)return route.fulfill({status:422,json:{error:'validation',message:'Save fixture failed'}});
      saved.set(id,request.postDataJSON());
    }
    value=phone(id);
  } else if(path==='/api/v1/phones')value=models.map(m=>({...phone(m.id),extensions:[]}));
  await route.fulfill({status:200,json:value});
});
const page = await context.newPage();
page.on('pageerror',e=>errors.push(e.message));
page.on('console',m=>{if(m.type()==='error'&&/Content Security Policy/.test(m.text()))errors.push(m.text());});
const keys=page.locator('[data-phone-keys]');
const dialog=()=>page.getByRole('dialog', { name:/Taste \d+ bearbeiten/ });
async function openModel(id) { await page.goto(`${base}/phones/${id}`); await keys.waitFor({timeout:20000}); }
async function apply() {await dialog().getByRole('button',{name:'Übernehmen',exact:true}).click();await dialog().waitFor({state:'detached'});}
async function assertFits() { assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'no sideways page scroll'); }
try {
  for(const model of models) {
    await openModel(model.id);
    const seen=[];
    for(let p=0;p<model.layout.pages.length;p++) {
      if(p)await keys.getByRole('button',{name:`Seite ${p+1}`,exact:true}).click();
      seen.push(...await keys.locator('[data-key]').evaluateAll(nodes=>nodes.map(n=>Number(n.dataset.key))));
    }
    assert.deepEqual(seen.sort((a,b)=>a-b),Array.from({length:model.line_keys},(_,i)=>i+1),model.id);
    const last=keys.locator(`[data-key="${model.line_keys}"]`);
    await last.click();
    await dialog().waitFor();
    await page.keyboard.press('Escape');
    await dialog().waitFor({state:'detached'});
    assert.ok(await last.evaluate(el=>el===document.activeElement),'return focus to selected key');
    await assertFits();
    if(screenshots)await keys.screenshot({path:join(screenshots,`${model.id}.png`)});
  }
  await openModel('t41p');
  await keys.locator('[data-key="4"]').click();
  const d=dialog();
  assert.equal(await d.getByLabel('Telefonbuch',{exact:true}).locator('option[value="private"]').count(),0);
  assert.equal(await d.getByRole('button',{name:/Privater Kontakt/}).count(),0);
  await d.getByLabel('Telefonbuch',{exact:true}).selectOption('suppliers');
  await d.getByLabel('Kontakt suchen').fill('Elektro');
  await d.getByRole('button',{name:/Elektro Beispiel/}).click();
  await d.getByLabel('Rufnummer oder Funktionscode',{exact:true}).selectOption('+491705550101');
  await d.getByLabel('Beschriftung',{exact:true}).fill('Elektriker mobil');
  if(screenshots)await d.screenshot({path:join(screenshots,'contact-dialog.png')});
  await apply();
  assert.match(await keys.locator('[data-key="4"]').innerText(),/Elektriker mobil/);
  // Apply is local; cancel must keep that draft and must not write to the API.
  assert.equal(writes.length,0);
  await keys.locator('[data-key="4"]').click();
  await dialog().getByLabel('Beschriftung',{exact:true}).fill('Discard me');
  await dialog().getByRole('button',{name:'Abbrechen',exact:true}).click();
  assert.match(await keys.locator('[data-key="4"]').innerText(),/Elektriker mobil/);
  await page.getByRole('button',{name:'Speichern & Resync',exact:true}).click();
  await page.getByText('Gespeichert. Das Telefon wurde zum Neuladen der Konfiguration benachrichtigt.').waitFor();
  assert.deepEqual(writes.map(w=>w.type),['save','resync']);
  assert.equal(writes[0].input.line_keys.find(k=>k.key===4).value,'+491705550101');
  await page.reload();await keys.waitFor();
  assert.match(await keys.locator('[data-key="4"]').innerText(),/Elektriker mobil/);
  await keys.locator('[data-key="5"]').click();
  await dialog().getByRole('tab',{name:'Nebenstelle',exact:true}).click();
  await dialog().getByLabel('Nebenstelle',{exact:true}).selectOption('103');
  await apply();
  await keys.locator('[data-key="3"]').click();
  await dialog().getByLabel('Nebenstelle',{exact:true}).selectOption('103');
  await apply();
  await page.getByRole('button',{name:'Speichern',exact:true}).click();
  await page.getByText(/Gespeichert.*Resync/i).first().waitFor();
  await page.locator('button:not([disabled])').filter({hasText:/^Speichern$/}).waitFor();
  assert.equal(saved.get('t41p').line_keys.find(k=>k.key===5).value,'**103');
  assert.equal(saved.get('t41p').line_keys.find(k=>k.key===3).value,'103');
  assert.equal(writes.at(-1).type,'save');
  // A failed save must not trigger a resync. A failed resync must not lose the save.
  failSave=true;const before=writes.length;
  await page.getByRole('button',{name:'Speichern & Resync',exact:true}).click();
  await page.getByText('Save fixture failed',{exact:false}).waitFor();
  assert.deepEqual(writes.slice(before).map(w=>w.type),['save']);
  failSave=false;failResync=true;
  await page.getByRole('button',{name:'Speichern & Resync',exact:true}).click();
  await page.getByText(/Gespeichert, aber die Resync-Anforderung/).waitFor();
  failResync=false;notified=0;
  await page.getByRole('button',{name:'Speichern & Resync',exact:true}).click();
  await page.getByText(/Gespeichert. Das Telefon ist offline/).waitFor();
  // Reset and explicit unused differ for a default line.
  await keys.locator('[data-key="1"]').click();
  await dialog().getByLabel('Typ',{exact:true}).selectOption('none');
  await apply();
  assert.match(await keys.locator('[data-key="1"]').innerText(),/Unbelegt/);
  await keys.locator('[data-key="1"]').click();
  await dialog().getByRole('button',{name:'Standard',exact:true}).click();
  assert.match(await keys.locator('[data-key="1"]').innerText(),/101/);
  // Missing account remains visible and cannot become a new active assignment.
  saved.set('t41p',{...saved.get('t41p'),line_keys:[{key:4,type:'speed_dial',value:'+4930',label:'Old account',account:6}]});
  await openModel('t41p');await keys.locator('[data-key="4"]').click();
  await dialog().getByRole('button',{name:'Übernehmen',exact:true}).click();
  await dialog().getByRole('alert').getByText(/zugewiesenes Konto/).waitFor();
  await page.keyboard.press('Escape');
  // Unchanged legacy overrides remain visible in the correction area.
  saved.set('t46u',{line_keys:[{key:40,type:'speed_dial',value:'+4930',label:'Legacy key',account:1}]});
  await openModel('t46u');await keys.getByText(/Legacy key/).waitFor();
  await page.getByRole('button',{name:'Speichern',exact:true}).click();
  await page.locator('button:not([disabled])').filter({hasText:/^Speichern$/}).waitFor();
  assert.equal(saved.get('t46u').line_keys[0].key,40);
  // Responsive light/dark layouts, including a touch overview and the contact dialog.
  for(const width of [320,768,1280])for(const theme of ['light','dark']) {
    await page.setViewportSize({width,height:950});
    await page.evaluate(value=>{localStorage.setItem('talkops.theme',value);document.documentElement.classList.toggle('dark',value==='dark');},theme);
    for(const id of ['t33g','t41p','t85w','t87w']) {await openModel(id);await assertFits();}
    await openModel('t41p');await keys.locator('[data-key="4"]').click();
    await dialog().getByRole('tab',{name:'Telefonbuch',exact:true}).click();
    await assertFits();
    assert.ok(await dialog().evaluate(el=>{const r=el.getBoundingClientRect();return r.top>=0&&r.bottom<=innerHeight+1&&r.left>=0&&r.right<=innerWidth+1;}),'dialog stays in viewport');
    await dialog().getByRole('button',{name:'Übernehmen',exact:true}).scrollIntoViewIfNeeded();
    if(screenshots)await page.screenshot({path:join(screenshots,`dialog-${width}-${theme}.png`)});
    await page.keyboard.press('Escape');
  }
  // A viewer has no edit or save controls with mutation capability.
  role='operator';await openModel('t41p');
  assert.equal(await keys.locator('[data-key]:not([disabled])').count(),0);
  assert.equal(await page.getByRole('button',{name:'Speichern & Resync',exact:true}).count(),0);
  assert.deepEqual(errors,[]);
  console.log('phone key browser checks passed: 21 models, contacts, persistence, save/resync failures, legacy values, roles and responsive themes');
} finally {await browser.close();}
