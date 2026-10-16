import assert from 'node:assert/strict';
import test from 'node:test';
import {
	defaultConfig,
	describe,
	hasDay,
	isArchive,
	joinPath,
	parentPath,
	runTimes,
	secretChange,
	targetKinds,
	toggleDay
} from '../src/lib/backupTargets.ts';

test('schedules list their run times and weekdays', () => {
	assert.deepEqual(runTimes({ weekdays: 127, hour: 3, minute: 30, every_hours: 0 }), ['03:30']);
	assert.deepEqual(runTimes({ weekdays: 127, hour: 1, minute: 0, every_hours: 6 }), [
		'01:00',
		'07:00',
		'13:00',
		'19:00'
	]);
	assert.equal(hasDay(0b1, 0), true);
	assert.equal(hasDay(0b1, 6), false);
	assert.equal(toggleDay(127, 6), 63);
	assert.equal(toggleDay(63, 6), 127);
});

test('the folder browser joins and leaves folders', () => {
	assert.equal(joinPath('', 'bucket'), 'bucket');
	assert.equal(joinPath('bucket/', 'talkops'), 'bucket/talkops');
	assert.equal(joinPath('', '/var/lib/talkops/targets'), '/var/lib/talkops/targets');
	assert.equal(parentPath('bucket/talkops', 's3'), 'bucket');
	assert.equal(parentPath('bucket', 's3'), '');
	assert.equal(
		parentPath('/var/lib/talkops/targets/usb', 'local', ['/var/lib/talkops/targets']),
		'/var/lib/talkops/targets'
	);
	assert.equal(parentPath('/var/lib/talkops/targets', 'local', ['/var/lib/talkops/targets']), '');
	assert.equal(parentPath('/volume1', 'sftp'), '/');
	assert.equal(parentPath('/volume1/backup', 'sftp'), '/volume1');
});

test('archive names, secrets and descriptions', () => {
	assert.equal(isArchive('talkops-20261015-033000.tar.gz.age'), true);
	assert.equal(isArchive('talkops-20261015-033000.tar.gz'), true);
	assert.equal(isArchive('talkops-20261015-033000.tar.gz.age.partial'), false);
	assert.equal(isArchive('notes.txt'), false);
	assert.equal(secretChange({ password: '' }), undefined);
	assert.deepEqual(secretChange({ password: 'x', private_key: '' }), { password: 'x' });
	assert.equal(describe(defaultConfig('local'), '/mnt/usb'), '/mnt/usb');
	assert.equal(
		describe({ kind: 'smb', host: 'nas', port: null, user: '', domain: '' }, 'backup/talkops'),
		'smb://nas/backup/talkops'
	);
	for (const kind of targetKinds) assert.equal(defaultConfig(kind).kind, kind);
});
