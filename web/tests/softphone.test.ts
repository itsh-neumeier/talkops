import assert from 'node:assert/strict';
import test from 'node:test';
import { callbackNumber, callTarget, missedIncoming, offersVideo } from '../src/lib/softphone.ts';
import type { Call } from '../src/lib/api.ts';

test('numbers and SIP addresses retain their intended destinations', () => {
	assert.deepEqual(callTarget(' +49 (89) 123-456 '), { display: '+4989123456', uri: null });
	assert.deepEqual(callTarget('sip:test.echo@SIP5060.NET'), {
		display: 'test.echo@SIP5060.NET',
		uri: 'sip:test.echo@sip5060.net'
	});
	assert.deepEqual(callTarget('*97'), { display: '*97', uri: null });
	for (const input of ['', 'sip:alice@example.com?header=x', 'alice@host:5060', '9'.repeat(321)]) {
		assert.equal(callTarget(input), null);
	}
});

test('callback and missed-call filtering use the selected side of internal calls', () => {
	const outgoing = {
		extension_id: 'alice',
		destination: '201',
		caller_number: '200',
		answered_at: null
	} as Call;
	assert.equal(callbackNumber(outgoing, 'alice'), '201');
	assert.equal(missedIncoming(outgoing, 'alice'), false);
	assert.equal(callbackNumber(outgoing, 'bob'), '200');
	assert.equal(missedIncoming(outgoing, 'bob'), true);
	assert.equal(missedIncoming({ ...outgoing, answered_at: '2026-10-12T12:00:00Z' }, 'bob'), false);
	const incoming = { ...outgoing, extension_id: null, caller_number: '+491234' };
	assert.equal(callbackNumber(incoming, 'alice'), '+491234');
	assert.equal(missedIncoming(incoming, 'alice'), true);
	const inboundWithOwner = { ...outgoing, direction: 'inbound' as const, caller_number: '+49555' };
	assert.equal(callbackNumber(inboundWithOwner, 'alice'), '+49555');
	assert.equal(missedIncoming(inboundWithOwner, 'alice'), true);
});

test('only an offer with an active video stream allows answering with video', () => {
	const audio = 'v=0\r\nm=audio 16384 RTP/SAVPF 111 0\r\na=rtpmap:111 opus/48000/2\r\n';
	assert.equal(offersVideo(audio), false);
	assert.equal(offersVideo(`${audio}m=video 16386 UDP/TLS/RTP/SAVPF 102\r\n`), true);
	// A rejected or disabled video stream has port 0.
	assert.equal(offersVideo(`${audio}m=video 0 UDP/TLS/RTP/SAVPF 102\r\n`), false);
	assert.equal(offersVideo(undefined), false);
	assert.equal(offersVideo(''), false);
});
