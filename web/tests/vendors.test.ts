import { test } from 'node:test';
import assert from 'node:assert/strict';
import { modelLabel, sortModels, vendorName } from '../src/lib/vendors.ts';

test('manufacturer names use their own spelling', () => {
	assert.equal(vendorName('cisco'), 'Cisco');
	assert.equal(vendorName('yealink'), 'Yealink');
	assert.equal(vendorName('2n'), '2N');
	assert.equal(vendorName('atcom'), 'ATCOM');
	assert.equal(vendorName('vtech'), 'VTech');
	assert.equal(vendorName('newvendor'), 'Newvendor');
	assert.equal(vendorName(undefined), '');
	assert.equal(modelLabel({ vendor: 'yealink', name: 'SIP-T41P' }), 'Yealink SIP-T41P');
	assert.equal(modelLabel({ vendor: 'yealink', name: 'Yealink T54W' }), 'Yealink T54W');
	assert.equal(modelLabel({ vendor: '', name: 'T54W' }), 'T54W');
});

test('models sort by manufacturer, then by model with numbers in order', () => {
	const sorted = sortModels([
		{ vendor: 'yealink', name: 'AX86R' },
		{ vendor: 'cisco', name: '8811' },
		{ vendor: 'avaya', name: 'J169' },
		{ vendor: 'cisco', name: '7861 MPP / 3PCC' },
		{ vendor: 'cisco', name: '7811 MPP / 3PCC' }
	]);
	assert.deepEqual(sorted.map(modelLabel), [
		'Avaya J169',
		'Cisco 7811 MPP / 3PCC',
		'Cisco 7861 MPP / 3PCC',
		'Cisco 8811',
		'Yealink AX86R'
	]);
});
