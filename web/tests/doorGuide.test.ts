import assert from 'node:assert/strict';
import test from 'node:test';
import de from '../src/lib/i18n/de.ts';
import en from '../src/lib/i18n/en.ts';
import { guideHash, parseGuideHash, sectionKeys } from '../src/lib/doors/guide.ts';
import { commonSections, doorVendors, vendorGuides } from '../src/lib/doors/vendors.ts';

test('the guide hash selects vendor and device with fallbacks', () => {
	assert.equal(parseGuideHash('', vendorGuides), null);
	assert.equal(parseGuideHash('#stations', vendorGuides), null);
	assert.deepEqual(parseGuideHash('#guide', vendorGuides), { vendor: 'dahua', device: 'vto' });
	assert.deepEqual(parseGuideHash('#guide/dahua/vth', vendorGuides), {
		vendor: 'dahua',
		device: 'vth'
	});
	assert.deepEqual(parseGuideHash('#guide/acme/x', vendorGuides), {
		vendor: 'dahua',
		device: 'vto'
	});
	assert.equal(guideHash({ vendor: 'dahua', device: 'vth' }), '#guide/dahua/vth');
	assert.equal(parseGuideHash('#guide', []), null);
});

test('vendors have unique ids, devices and section ids', () => {
	assert.deepEqual(
		doorVendors.map((v) => v.id),
		['dahua']
	);
	for (const v of vendorGuides) {
		assert.ok(v.devices.length > 0, v.id);
		assert.equal(new Set(v.devices.map((d) => d.id)).size, v.devices.length);
		for (const d of v.devices) {
			assert.equal(new Set(d.sections.map((s) => s.id)).size, d.sections.length, d.id);
		}
	}
	assert.equal(new Set(commonSections.map((s) => s.id)).size, commonSections.length);
});

test('every guide text exists in English and German', () => {
	const keys = new Set(sectionKeys(commonSections));
	for (const v of vendorGuides) {
		for (const d of v.devices) {
			keys.add(d.label);
			keys.add(d.summary);
			sectionKeys(d.sections).forEach((k) => keys.add(k));
		}
	}
	const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
	for (const key of keys) {
		assert.ok(en[key]?.trim(), `en: ${key}`);
		assert.ok(de[key]?.trim(), `de: ${key}`);
		for (const p of placeholders(en[key])) assert.ok(['host', 'port', 'domain'].includes(p), key);
		assert.deepEqual(placeholders(de[key]), placeholders(en[key]), key);
	}
	// The SIP user name of an indoor monitor is its room number.
	assert.match(en['doorGuide.dahua.vth.prereq.room'], /Room No\./);
	assert.match(de['doorGuide.dahua.vth.prereq.room'], /Room No\./);
});
