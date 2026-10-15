// Setup assistant interaction test. API integration is covered by Rust tests.
import assert from 'node:assert/strict';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ?? 'playwright');
const base = process.argv[2] ?? 'http://127.0.0.1:4178';
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
try {
  const context = await browser.newContext({ locale: 'en-US' });
  await context.addInitScript(() => localStorage.setItem('talkops.locale', 'en'));
  const page = await context.newPage();
  let restored = false;
  let inspected = false;
  let complete = false;
  const browsed = [];
  await page.route('**/api/v1/**', async route => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/v1/setup/restore/inspect') {
      assert.equal(route.request().headers()['x-requested-with'], 'TalkOps');
      assert.equal(route.request().headers()['x-talkops-setup-token'], 'local-setup-token-with-32-characters');
      inspected = true;
      return route.fulfill({json:{manifest:{format:1,talkops_version:'1.7.5',created_at:'2026-10-12T00:00:00Z',schema_version:0,volumes:['voicemail'],deployment:complete ? {complete:true,missing:[],files:['.env','docker-compose.yml'],source:'imported',captured_at:'2026-10-12T00:00:00Z',project_name:'talkops',profiles:'caddy'} : null},deployment_bundle:complete ? Buffer.from('fixture').toString('base64') : null}});
    }
    if (path === '/api/v1/setup/restore/target/browse') {
      assert.equal(route.request().headers()['x-talkops-setup-token'], 'local-setup-token-with-32-characters');
      const body = route.request().postDataJSON();
      assert.equal(body.config.kind, 'webdav');
      assert.deepEqual(body.secret, {password:'dav-password'});
      browsed.push(body.path);
      const entries = body.path === 'talkops'
        ? [{name:'talkops-20261015-033000.tar.gz.age',is_dir:false,size:10,modified:'2026-10-15T03:30:00Z'},{name:'notes.txt',is_dir:false,size:1,modified:null}]
        : [{name:'talkops',is_dir:true,size:0,modified:null}];
      return route.fulfill({json:{entries,truncated:false,host_keys:null,fingerprint:null}});
    }
    if (path === '/api/v1/setup/restore/target/inspect') {
      const body = route.request().postDataJSON();
      assert.equal(body.name, 'talkops-20261015-033000.tar.gz.age');
      assert.equal(body.connection.path, 'talkops');
      assert.equal(body.password, 'backup-password');
      return route.fulfill({json:{manifest:{format:2,talkops_version:'1.7.13',created_at:'2026-10-15T03:30:00Z',schema_version:20261102000001,volumes:['voicemail','recordings'],deployment:null},deployment_bundle:null,archive:body.name}});
    }
    if (path === '/api/v1/setup') return route.fulfill({json:{needs_setup:!restored}});
    return route.fulfill({status:401,json:{error:'unauthorized',message:'login required'}});
  });
  await page.goto(`${base}/setup`);
  await page.getByRole('button',{name:'Restore from backup',exact:true}).click();
  await page.locator('#restore-token').fill('local-setup-token-with-32-characters');
  await page.locator('#restore-file').setInputFiles({name:'backup.tar.gz',mimeType:'application/gzip',buffer:Buffer.from('fixture')});
  await page.getByRole('button',{name:'Inspect backup',exact:true}).click();
  await page.getByText('This old backup contains no deployment configuration.',{exact:false}).waitFor();
  assert.ok(inspected);
  await page.getByText('This backup excludes call recordings.',{exact:false}).waitFor();
  await page.getByRole('button',{name:'Check completion and go to login'}).click();
  await page.getByText('No restored account is available yet.',{exact:false}).waitFor();
  assert.ok(page.url().endsWith('/setup'));
  restored = true;
  await page.getByRole('button',{name:'Check completion and go to login'}).click();
  await page.waitForURL(`${base}/login`);
  restored = false; complete = true;
  await page.goto(base + '/setup');
  await page.getByRole('button',{name:'Restore from backup',exact:true}).click();
  await page.locator('#restore-token').fill('local-setup-token-with-32-characters');
  await page.locator('#restore-file').setInputFiles({name:'backup.age',mimeType:'application/octet-stream',buffer:Buffer.from('fixture')});
  await page.getByRole('button',{name:'Inspect backup',exact:true}).click();
  await page.getByText('The required deployment files and credentials are included.',{exact:true}).waitFor();
  const button = page.getByRole('button',{name:'Download deployment files',exact:true});
  assert.equal(await button.isEnabled(),false);
  await page.getByRole('checkbox').check();
  const downloading = page.waitForEvent('download'); await button.click();
  assert.equal((await downloading).suggestedFilename(),'talkops-deployment.tar.gz');
  // Restore source: a backup target (WebDAV) with the folder browser.
  await page.goto(base + '/setup');
  await page.getByRole('button',{name:'Restore from backup',exact:true}).click();
  await page.locator('#restore-token').fill('local-setup-token-with-32-characters');
  await page.getByRole('tab',{name:'From a backup target'}).click();
  await page.locator('#rt-kind').selectOption('webdav');
  await page.locator('#rt-url').fill('https://cloud.example.com/remote.php/dav/files/u');
  await page.locator('#rt-user').fill('u');
  await page.locator('#rt-password').fill('dav-password');
  await page.getByRole('button',{name:'Browse',exact:true}).click();
  await page.getByRole('button',{name:/talkops$/}).click();
  await page.getByRole('button',{name:'talkops-20261015-033000.tar.gz.age'}).click();
  assert.deepEqual(browsed, ['', 'talkops']);
  assert.equal(await page.getByRole('button',{name:'notes.txt'}).count(), 0);
  await page.locator('#rt-backup-password').fill('backup-password');
  await page.getByRole('button',{name:'Inspect backup',exact:true}).click();
  await page.getByText('The archive is now in the backups volume',{exact:false}).waitFor();
  await page.getByText('recover.sh restore --backup talkops-20261015-033000.tar.gz.age',{exact:false}).waitFor();
  await page.goto(base + '/setup');
  await page.getByRole('button',{name:'Set up a new instance',exact:true}).click();
  await page.locator('#username').waitFor();
  console.log('Passed: restore choice, upload authorization, legacy guidance, omitted media, completion, backup target source and new-instance choice');
} finally { await browser.close(); }
