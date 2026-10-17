// Model fields, locale, secrets and same-route copy navigation on the built UI.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url);
const {chromium}=require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/,'index.js')||'playwright');
const browser=await chromium.launch({headless:true,...(process.env.BROWSER_EXECUTABLE?{executablePath:process.env.BROWSER_EXECUTABLE}:{})});
const base=process.env.BROWSER_BASE_URL||'http://127.0.0.1:5184';
const model={id:'fanvil-x4',name:'X4',vendor:'fanvil',family:'desk',accounts:2,line_keys:0,line_key_limit:0,layout:null,video:false,wallpaper:false,ringtone_max_kb:0,
 provisioning_capabilities:{native:true,adaptation:'adapted',device_validation:'not_tested',account_capacity_source:'source_slots',identity_kind:'mac',resync:false,reboot:false,
 settings:[{key:'fanvil_time_zone',label:'time zone',kind:'text',copyable:true}],key_groups:['line'],key_functions:[{uuid:'native-blf',type:'blf',subtype:'',value:'b',description:'Monitor another extension.'}]}};
const field=(key,category,extra={})=>({key,parameter:key,artifact:'{$mac}.cfg',category,description:'Source description '+key,kind:'text',managed:false,copyable:true,default:'0',...extra});
const fields=[field('display.brightness','display'),field('web.password','security',{kind:'secret',copyable:false,default:null}),field('sip.userid','sip',{managed:true,copyable:false,default:null}),...Array.from({length:45},(_,i)=>field('display.option'+i,'display'))];
let phone={id:'source',name:'Source phone',mac:'805ec0010203',model:model.id,line_keys:[],settings:{},phonebook_sections:[],ringtone_id:null,wallpaper_id:null,accounts:[],last_seen_at:null,last_ip:null,last_firmware:null,
 provisioning:{language:'en',parameters:{'web.password':'REDACTED'}},provisioning_base:'https://pbx.example/provisioning/devices/805ec0010203/',provisioning_files:[{artifact:'{$mac}.cfg',format:'xml',name:'805ec0010203.cfg',url:'https://pbx.example/provisioning/devices/805ec0010203/805ec0010203.cfg',primary:true}]};
let copy,cloneInput,parameterRequests=0;const errors=[],writes=[];
const context=await browser.newContext({locale:'de-DE',reducedMotion:'reduce',viewport:{width:1280,height:1000}});
await context.addInitScript(()=>{localStorage.setItem('talkops.locale','de');localStorage.setItem('talkops.gettingStarted.hidden','1');});
await context.route('**/api/**',async route=>{
 const req=route.request(),path=new URL(req.url()).pathname,method=req.method();let value=[];
 if(path==='/api/v1/auth/me')value={user:{id:'admin',username:'admin',display_name:'Admin',role:'admin',enabled:true},csrf_token:'fixture',permissions:[]};
 else if(path==='/api/v1/status')value={version:'test',database:{ok:true},freeswitch:{ok:true}};
 else if(path==='/api/v1/phone-models')value=[model];
 else if(path.endsWith('/parameters')){parameterRequests++;value={fields,coverage:'source_templates'};}
 else if(path==='/api/v1/phone-settings')value={catalog:[],values:{}};
 else if(path==='/api/v1/extensions')value=[{id:'ext',number:'101',display_name:'Office'}];
 else if(path==='/api/v1/phones/source/clone-preview')value={source_revision:'revision',configuration:{...phone,provisioning:{...phone.provisioning,parameters:{'display.brightness':'7'}}},excluded:['id','mac','sip_credentials','web.password'],http_exception:false};
 else if(path==='/api/v1/phones/source/clone'){cloneInput=req.postDataJSON();copy={...phone,id:'copy',name:cloneInput.name,mac:cloneInput.mac,accounts:[]};value=copy;}
 else if(path==='/api/v1/phones/source'){
   if(method==='PUT'){phone={...phone,...req.postDataJSON()};writes.push(phone);}value=phone;
 }else if(path==='/api/v1/phones/copy')value=copy;
 else if(path==='/api/v1/phones')value=[phone];
 else if(path.endsWith('/config'))return route.fulfill({status:200,contentType:'text/plain',body:'display.brightness = 7'});
 await route.fulfill({status:200,json:value});
});
const page=await context.newPage();page.on('pageerror',error=>errors.push(error.message));
try{
 await page.goto(base+'/phones/source');
 await page.getByRole('heading',{name:'Modelleinstellungen · fanvil X4'}).waitFor();
 assert.equal(await page.getByRole('button',{name:'Speichern & Resync',exact:true}).count(),0);
 await page.getByRole('combobox',{name:/Telefonsprache/}).selectOption('de');
 await page.getByLabel('Zeitzone (IANA)',{exact:true}).fill('Europe/Berlin');
 await page.getByLabel('NTP-Server',{exact:true}).fill('time.example.org');
 await page.getByText('Display (46)',{exact:false}).click();
 await page.getByLabel('display.brightness',{exact:true}).fill('');
 await page.getByLabel('display.brightness',{exact:true}).pressSequentially('7.25',{delay:40});
 assert.equal(await page.getByLabel('display.brightness',{exact:true}).inputValue(),'7.25');
 assert.equal(parameterRequests,1,'editing values must preserve the field schema and input focus');
 await page.getByRole('button',{name:/Weitere Felder anzeigen/}).click();
 await page.getByLabel('display.option44',{exact:true}).waitFor();
 await page.getByLabel('Einstellung, Parameter oder Quelldatei suchen').fill('web.password');
 const password=page.getByLabel('web.password',{exact:true});await password.waitFor();assert.equal(await password.inputValue(),'');
 assert((await password.getAttribute('placeholder')).includes('Geheimnis gespeichert'));
 await page.getByLabel('Einstellung, Parameter oder Quelldatei suchen').fill('sip.userid');
 await page.getByText('Über TalkOps-Konten, Geräteidentität oder Provisionierungseinstellungen verwaltet.').waitFor();
 assert.equal(await page.getByLabel('sip.userid',{exact:true}).count(),0);
 await page.getByLabel('Einstellung, Parameter oder Quelldatei suchen').fill('');
 await page.getByRole('button',{name:'Tastenbelegung hinzufügen'}).click();
 await page.getByRole('combobox',{name:/Typ/}).selectOption('native-blf');
 await page.getByText('Monitor another extension.').waitFor();
 assert.equal(await page.getByText(/Diese gespeicherten Belegungen überschreiten/).count(),0,'native keys must not be classified as legacy Yealink overrides');
 await Promise.all([page.waitForResponse(r=>r.request().method()==='PUT'&&r.url().endsWith('/phones/source')),page.getByRole('button',{name:'Speichern',exact:true}).last().click()]);
 assert.equal(writes.at(-1).provisioning.language,'de');assert.equal(writes.at(-1).provisioning.time_zone,'Europe/Berlin');
 assert.equal(writes.at(-1).provisioning.parameters['display.brightness'],'7.25');assert.equal(writes.at(-1).provisioning.parameters['web.password'],'REDACTED');
 assert.equal(writes.at(-1).provisioning.keys[0].native_function,'native-blf');
 await page.getByRole('button',{name:'Einstellungen von einem Telefon übernehmen',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'Einstellungen von einem Telefon übernehmen'});
 await dialog.getByLabel('Name',{exact:true}).fill('New office');await dialog.getByLabel('MAC-Adresse',{exact:true}).fill('805ec0040506');
 await dialog.getByLabel('Konto 2',{exact:true}).selectOption('ext');
 await dialog.getByRole('button',{name:'Telefon mit diesen Einstellungen anlegen'}).click();
 await page.waitForURL('**/phones/copy');await page.getByRole('heading',{level:1,name:'New office'}).waitFor();
 assert.equal(cloneInput.source_revision,'revision');assert.deepEqual(cloneInput.assignments,[{account_index:2,extension_id:'ext'}]);
 assert.equal(await page.getByLabel('Name',{exact:true}).inputValue(),'New office');
 for(const width of [320,768,1280])for(const dark of [false,true]){
   await page.setViewportSize({width,height:1000});await page.evaluate(d=>document.documentElement.classList.toggle('dark',d),dark);
   await page.getByLabel('Einstellung, Parameter oder Quelldatei suchen').fill('display');
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'no horizontal page overflow');
 }
 assert.deepEqual(errors,[]);console.log('Manufacturer browser checks passed: categorized fields, pagination, search, locale, managed values, secret preservation, native key functions, save, copy assignments, same-route navigation and 320/768/1280 light/dark layouts.');
}catch(error){console.error('Page:',page.url(),'Errors:',errors,'Body:',(await page.locator('body').innerText()).slice(-8000));throw error;}
finally{await browser.close();}
