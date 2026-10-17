// Provisioning forms, transport exceptions and versioned template flows on the production UI.
import assert from 'node:assert/strict';
import { readFileSync,mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require=createRequire(import.meta.url);
const {chromium}=require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/,'index.js')||'playwright');
const browser=await chromium.launch({headless:true,...(process.env.BROWSER_EXECUTABLE?{executablePath:process.env.BROWSER_EXECUTABLE}:{})});
const base=process.env.TALKOPS_UI_URL||'http://127.0.0.1:5184';
const caps=JSON.parse(readFileSync(new URL('../../crates/talkops-provisioning/profiles/yealink.json',import.meta.url),'utf8'));
const source=JSON.parse(readFileSync(new URL('../../docs/research/yealink-template-inventory.json',import.meta.url),'utf8'));
const native={'common.cfg':'#!version:1.0.0.1\n## native common\n','mac.cfg':'#!version:1.0.0.1\n## native device\n'};
const models=[{id:'t41p',name:'SIP-T41P',vendor:'yealink',family:'desk',accounts:6,line_keys:15,line_key_limit:6,layout:null,video:false,wallpaper:false,ringtone_max_kb:0,speed_dial_keys:true,key_capabilities:caps.t41p}];
const account={device_id:'dev1',extension_id:'ext1',extension_number:'101',display_name:'Office',phone_label:'',phone_display_name:'',account_index:1};
let phone={id:'t41p',name:'Office',mac:'805ec0aabbcc',model:'t41p',line_keys:[],settings:{},provisioning:{},phonebook_sections:[],ringtone_id:null,wallpaper_id:null,last_seen_at:null,last_ip:null,last_firmware:'36.83.0.120',accounts:[account]};
let templates=[],revisions=[],failActivation=false,failSave=false,role='admin';
const writes=[],errors=[];
const context=await browser.newContext({locale:'de-DE',reducedMotion:'reduce',viewport:{width:1280,height:1000}});
await context.addInitScript(()=>{localStorage.setItem('talkops.locale','de');localStorage.setItem('talkops.gettingStarted.hidden','1');});
await context.route('**/api/**',async route=>{
 const req=route.request(),path=new URL(req.url()).pathname,method=req.method();let value=[];
 if(path==='/api/v1/auth/me')value={user:{id:'admin',username:'admin',display_name:'Admin',role,enabled:true},csrf_token:'fixture',permissions:['phones.view']};
 else if(path==='/api/v1/status')value={version:'test',database:{ok:true},freeswitch:{ok:true}};
 else if(path==='/api/v1/phone-models')value=models;
 else if(path==='/api/v1/phone-settings')value={catalog:[],values:{}};
 else if(path==='/api/v1/contacts')value=[{id:'contact',name:'Example',company:'',section_id:null,phone_work:'+49305550101',phone_mobile:'+491705550101',phone_other:''}];
 else if(path==='/api/v1/phones/t41p/preview')value={before:'secret = REDACTED\nkey = old',after:'secret = REDACTED\nkey = new'};
 else if(path==='/api/v1/phones/t41p/resync'){writes.push('resync');value={notified:1};}
 else if(path==='/api/v1/phones/t41p'){
   if(method==='PUT'){if(failSave)return route.fulfill({status:422,json:{message:'Save failed'}});phone={...phone,...req.postDataJSON()};writes.push('save');}value=phone;
 }else if(path==='/api/v1/phones')value=[{...phone,extensions:[]}];
 else if(path==='/api/v1/provisioning/options')value={endpoint:'https://secure.example/provisioning',sip_host:'192.168.1.10'};
 else if(path==='/api/v1/provisioning/requests')value=[{mac:phone.mac,filename:phone.mac+'.cfg',scheme:'http',status:200,created_at:'2026-10-16T20:00:00Z'}];
 else if(path==='/api/v1/provisioning/library')value={profiles:source.models.map(m=>({model:m.id,vendor:'yealink',adapted:m.id==='t41p',files:m.files.map(p=>p.split('/'+m.id+'/')[1])}))};
 else if(path.startsWith('/api/v1/provisioning/library/source/'))return route.fulfill({status:200,contentType:'text/plain',body:'#!version:1.0.0.1\nprogramablekey.1.type = {$device_key_type}\n'});
 else if(path==='/api/v1/provisioning/templates'){
   if(method==='POST'){templates=[{id:'template',name:req.postDataJSON().name,model:'t41p',vendor:'yealink',active_revision:null}];revisions=[{revision:1,files:native,created_at:'2026-10-16T20:00:00Z'}];value=templates[0];}else value=templates;
 }else if(path==='/api/v1/provisioning/templates/template/revisions'){
   if(method==='POST'){const revision=revisions.length+1;revisions=[{revision,files:req.postDataJSON().files,created_at:'2026-10-16T20:00:00Z'},...revisions];value={revision};}else value=revisions;
 }else if(path==='/api/v1/provisioning/templates/template/activate'){
   if(failActivation)return route.fulfill({status:422,json:{message:'Undefined template variable'}});
   templates[0].active_revision=req.postDataJSON().revision;return route.fulfill({status:204});
 }
 await route.fulfill({status:200,json:value});
});
const page=await context.newPage();page.on('pageerror',err=>errors.push(err.message));
const dialog=()=>page.getByRole('dialog',{name:'Funktionstaste bearbeiten'});
async function fits(){assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'no horizontal page overflow');}
try{
 await page.goto(base+'/phones/t41p');
 await page.getByRole('heading',{name:'Geräte-Provisionierung'}).waitFor();
 await page.getByLabel('Provisionierungsadresse für Telefone',{exact:true}).fill('http://192.168.1.10:8095/provisioning');
 await page.getByRole('button',{name:/Nachricht.*Werkseinstellung/}).click();
 await dialog().getByLabel('Typ',{exact:true}).selectOption('menu');
 assert.equal(await dialog().getByLabel('Konto',{exact:true}).count(),0,'menu has no account');
 await dialog().getByRole('button',{name:'Übernehmen',exact:true}).click();
 await page.getByRole('button',{name:/Nachricht.*Menü/}).waitFor();
 await page.getByRole('button',{name:/Nach oben.*Werkseinstellung/}).click();
 await dialog().getByLabel('Typ',{exact:true}).selectOption('speed_dial');
 await dialog().getByLabel('Telefonbuch',{exact:true}).selectOption('contact');
 await dialog().getByLabel('Rufnummer oder Funktionscode',{exact:true}).selectOption('+491705550101');
 await dialog().getByRole('button',{name:'Übernehmen',exact:true}).click();
 await Promise.all([page.waitForResponse(r=>r.url().endsWith('/phones/t41p/resync')),page.getByRole('button',{name:'Speichern & Resync',exact:true}).click()]);
 assert.deepEqual(writes.slice(-2),['save','resync']);
 assert.equal(phone.provisioning.endpoint,'http://192.168.1.10:8095/provisioning');
 assert(phone.provisioning.keys.some(k=>k.key===18&&k.type==='menu'&&k.account===0));
 assert(phone.provisioning.keys.some(k=>k.key===5&&k.value==='+491705550101'));
 await page.getByRole('button',{name:'Konfigurationsvorschau',exact:true}).click();
 await page.getByRole('dialog',{name:'Konfigurationsvorschau'}).getByText('+ key = new',{exact:false}).waitFor();
 await page.keyboard.press('Escape');
 await page.getByRole('tab',{name:'Softkeys im Gespräch',exact:true}).click();
 await page.getByLabel('Position 1',{exact:true}).first().selectOption('Answer');
 await Promise.all([page.waitForResponse(r=>r.request().method()==='PUT'&&r.url().endsWith('/phones/t41p')),page.getByRole('button',{name:'Speichern',exact:true}).last().click()]);
 assert.equal(phone.provisioning.softkeys.call_in[0],'Answer');
 await page.getByRole('tab',{name:'Programmierbare Tasten',exact:true}).click();
 await page.getByRole('button',{name:/Nachricht.*Menü/}).click();
 await dialog().getByLabel('Typ',{exact:true}).selectOption('inherit');
 await dialog().getByRole('button',{name:'Übernehmen',exact:true}).click();
 await Promise.all([page.waitForResponse(r=>r.request().method()==='PUT'&&r.url().endsWith('/phones/t41p')),page.getByRole('button',{name:'Speichern',exact:true}).last().click()]);
 assert(!phone.provisioning.keys.some(k=>k.key===18));
 failSave=true;const prior=writes.length;await page.getByRole('button',{name:'Speichern & Resync',exact:true}).click();await page.getByText('Save failed',{exact:false}).waitFor();assert.equal(writes.length,prior);failSave=false;
 await page.goto(base+'/phones/templates');
 await page.getByLabel('Modell',{exact:true}).waitFor();
 assert.equal(await page.getByLabel('Modell',{exact:true}).locator('option').count(),70);
 await page.getByRole('tab',{name:'TalkOps-Vorlagen',exact:true}).click();
 await page.getByLabel('Name',{exact:true}).fill('Test template');
 await page.getByRole('button',{name:'Kopieren',exact:true}).click();
 await page.getByLabel('Quelltext',{exact:true}).fill(native['mac.cfg']+'phone_setting.test = 1\n');
 assert(await page.getByRole('button',{name:'Prüfen und aktivieren',exact:true}).isDisabled());
 await page.getByRole('button',{name:'Neue Version speichern',exact:true}).click();
 await page.getByRole('button',{name:'Prüfen und aktivieren',exact:true}).click();
 await page.getByText('Vorlagenversion aktiviert.',{exact:false}).waitFor();assert.equal(templates[0].active_revision,2);
 failActivation=true;await page.getByLabel('Version',{exact:true}).selectOption('1');
 await page.getByRole('button',{name:'Prüfen und aktivieren',exact:true}).click();
 await page.getByText('Undefined template variable',{exact:false}).waitFor();assert.equal(templates[0].active_revision,2);
 for(const width of [320,768,1280])for(const dark of [false,true]){
   await page.setViewportSize({width,height:1000});await page.evaluate(d=>{localStorage.setItem('talkops.theme',d?'dark':'light');document.documentElement.classList.toggle('dark',d);},dark);
   for(const path of ['/phones/t41p','/phones/templates','/phones']){
     await page.goto(base+path);await page.getByRole('heading',{level:1}).waitFor();await fits();
     assert.equal(await page.evaluate(()=>document.documentElement.classList.contains('dark')),dark,'stored theme survives navigation');
     if(process.env.TALKOPS_SCREENSHOTS&&path==='/phones/t41p'&&((width===320&&dark)||(width===1280&&!dark))){
       const output=new URL('../../target/browser/',import.meta.url);mkdirSync(output,{recursive:true});
       await page.screenshot({path:fileURLToPath(new URL(`provisioning-${width}-${dark?'dark':'light'}.png`,output)),fullPage:true});
     }
   }
 }
 assert.deepEqual(errors,[]);
 console.log('Provisioning browser checks passed: T41P keys, contact/mobile target, transport override, save/resync ordering, failed saves, redacted diff, call states, reset, 70 source folders, immutable draft activation and failure preservation; 320/768/1280 light/dark layouts.');
}catch(error){
 console.error('Page:',page.url(),'Errors:',errors,'Body:',(await page.locator('body').innerText()).slice(-7000));
 mkdirSync(new URL('../../target/browser/',import.meta.url),{recursive:true});
 await page.screenshot({path:fileURLToPath(new URL('../../target/browser/provisioning-failure.png',import.meta.url)),fullPage:true});
 throw error;
}finally{await browser.close();}
