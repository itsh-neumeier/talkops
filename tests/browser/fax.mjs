import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url);
const {chromium}=require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/,'index.js') || 'playwright');
const browser=await chromium.launch({headless:true,...(process.env.BROWSER_EXECUTABLE ? {executablePath:process.env.BROWSER_EXECUTABLE}: {})});
const base=process.env.BASE || 'http://127.0.0.1:5184';
const at='2026-10-18T12:00:00Z';
const number={id:'n1',trunk_id:'t1',e164:'+49123456',label:'Fax',enabled:true,destination_type:'none',destination_id:null,extra_extensions:[]};
const config={enabled:true,global_number_id:'n1',always_global:false,header:'Header {page}',footer:'Footer',retention_days:30,imap_enabled:false,imap_host:'mail.example.com',imap_port:993,imap_username:'faxin',imap_folder:'INBOX',faxin_address:'faxin@example.com',trusted_mailbox:false,trunk_modes:{}};
let uploaded;
let faxes=[];
try {
  const context=await browser.newContext({locale:'en-US',viewport:{width:1280,height:900},reducedMotion:'reduce'});
  await context.addInitScript(()=>{localStorage.setItem('talkops.locale','en');localStorage.setItem('talkops.gettingStarted.hidden','1');});
  const page=await context.newPage();const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.route('**/api/**',async route=>{
    const req=route.request();const path=new URL(req.url()).pathname;
    if(path.endsWith('/document')) return route.fulfill({contentType:'application/pdf',body:'%PDF-1.4\n%%EOF'});
    if(path==='/api/v1/faxes' && req.method()==='POST') {
      uploaded=req.postDataJSON();
      const fax={id:'f1',owner_id:'u1',direction:'outbound',state:'draft',source:'+49123456',destination:uploaded.destination,sender_email:'',pages:2,result:'',created_at:at,attempts:0};
      faxes=[fax];return route.fulfill({json:fax});
    }
    if(path.endsWith('/submit')) {faxes[0].state='queued';return route.fulfill({json:faxes[0]});}
    const fixtures={
      '/api/v1/auth/me':{user:{id:'u1',username:'admin',display_name:'Admin',role:'admin',enabled:true},csrf_token:'test',permissions:[]},
      '/api/v1/setup':{needs_setup:false},'/api/v1/status':{database:{ok:true},freeswitch:{ok:true}},
      '/api/v1/faxes':faxes,'/api/v1/faxes/options':{enabled:true,numbers:[number]},
      '/api/v1/settings/fax':{config,has_password:true,numbers:[number],users:[{id:'u1',name:'Admin'}]},
      '/api/v1/settings/fax/whitelist':[{email:'allowed@example.com',user_id:'u1',enabled:true}],
      '/api/v1/settings/fax/input-log':{receipts:[],deliveries:[]},
      '/api/v1/fax-endpoints':[],'/api/v1/faxes/template':{header:'Header',footer:'Footer'}
    };
    return route.fulfill({json:fixtures[path] ?? []});
  });
  await page.goto(`${base}/fax`);
  await page.getByRole('tab',{name:'Send',exact:true}).click();
  await page.getByLabel('Destination number').fill('+49123456');
  await page.getByLabel('Attachments').setInputFiles([
    {name:'one.pdf',mimeType:'application/pdf',buffer:Buffer.from('%PDF-1.4')},
    {name:'two.png',mimeType:'image/png',buffer:Buffer.from('image fixture')}
  ]);
  await page.getByRole('button',{name:'Prepare preview',exact:true}).click();
  await page.getByRole('button',{name:'Send fax now'}).waitFor();
  assert.deepEqual(uploaded.files.map(f=>f.name),['one.pdf','two.png']);
  assert.equal(uploaded.destination,'+49123456');
  await page.getByRole('button',{name:'Send fax now'}).click();
  await page.getByRole('tab',{name:'Outbox'}).waitFor();
  await page.goto(`${base}/settings?section=fax`);
  await page.getByText('Sender whitelist',{exact:true}).waitFor();
  await page.getByText('The mail server restricts this mailbox',{exact:false}).waitFor();
  assert.equal(await page.getByRole('textbox',{name:'Email address',exact:true}).inputValue(),'allowed@example.com');
  await page.setViewportSize({width:390,height:844});
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1));
  if(process.env.SCREENSHOT) await page.screenshot({path:process.env.SCREENSHOT,fullPage:true});
  assert.deepEqual(errors,[]);
  await context.close();
  const denied=await browser.newContext();const restricted=await denied.newPage();
  await restricted.route('**/api/**',route=>route.fulfill({json:route.request().url().includes('/auth/me')?{user:{id:'u2',username:'user',display_name:'User',role:'user',enabled:true},permissions:[],csrf_token:'test'}:route.request().url().includes('/faxes/options')?{enabled:true,numbers:[]}:[]}));
  await restricted.goto(`${base}/fax`);await restricted.getByRole('tab',{name:/Outbox|Ausgang/}).waitFor();
  assert.equal(await restricted.getByRole('tab',{name:/^Send$|^Senden$/}).count(),0);
  await denied.close();
  console.log('Fax browser checks passed: attachments, preview, submit, whitelist, mobile layout and permissions.');
} finally {await browser.close();}
