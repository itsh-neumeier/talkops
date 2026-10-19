import type { Call } from './api.ts';
import { isBlocked } from './calls/blocking.ts';

/** A number or a SIP address; never let pasted text change the SIP URI structure. */
export function callTarget(input: string): { display: string; uri: string | null } | null {
	const raw = input.trim().replace(/^sip:/i, '');
	const addr = raw.match(/^([A-Za-z0-9._~!*'()+-]{1,64})@([A-Za-z0-9.-]{3,253})$/);
	if (addr) return { display: raw, uri: `sip:${addr[1]}@${addr[2].toLowerCase()}` };
	if (raw.includes('@') || raw.length > 320) return null;
	const digits = raw.replace(/[^0-9*#+]/g, '');
	return digits && digits.length <= 64 ? { display: digits, uri: null } : null;
}

/**
 * Whether an SDP offer has an active video stream. TalkOps offers video only
 * where the call allows it, so an audio-only offer gets no video button.
 */
export function offersVideo(sdp: string | undefined): boolean {
	return /^m=video +[1-9][0-9]* /m.test(sdp ?? '');
}

/** Inbound CDRs can identify the destination in either extension field. */
export function callbackNumber(call: Call, extensionId: string): string {
	return call.direction !== 'inbound' && call.extension_id === extensionId
		? call.destination
		: call.caller_number;
}

export function missedIncoming(call: Call, extensionId: string): boolean {
	return (
		!isBlocked(call) &&
		!call.answered_at &&
		(call.direction === 'inbound' || call.extension_id !== extensionId)
	);
}

/** Generate local PCM audio. Blob URLs comply with the existing media CSP. */
export function toneUrl(ringing = false): string {
	const rate = 8000;
	const count = rate * (ringing ? 3 : 0.4);
	const buffer = new ArrayBuffer(44 + count * 2);
	const view = new DataView(buffer);
	const write = (offset: number, value: string) => {
		for (let i = 0; i < value.length; i++) view.setUint8(offset + i, value.charCodeAt(i));
	};
	write(0, 'RIFF');
	view.setUint32(4, buffer.byteLength - 8, true);
	write(8, 'WAVE');
	write(12, 'fmt ');
	view.setUint32(16, 16, true);
	view.setUint16(20, 1, true);
	view.setUint16(22, 1, true);
	view.setUint32(24, rate, true);
	view.setUint32(28, rate * 2, true);
	view.setUint16(32, 2, true);
	view.setUint16(34, 16, true);
	write(36, 'data');
	view.setUint32(40, count * 2, true);
	for (let i = 0; i < count; i++) {
		const seconds = i / rate;
		const audible = !ringing || seconds < 1;
		const envelope = Math.min(1, seconds * 100, ((ringing ? 1 : 0.4) - seconds) * 100);
		view.setInt16(
			44 + i * 2,
			audible ? Math.sin(seconds * Math.PI * 2 * 440) * 5000 * Math.max(0, envelope) : 0,
			true
		);
	}
	return URL.createObjectURL(new Blob([buffer], { type: 'audio/wav' }));
}

export async function setAudioOutput(element: HTMLMediaElement, deviceId: string) {
	const output = element as HTMLMediaElement & { setSinkId?: (id: string) => Promise<void> };
	if (output.setSinkId) await output.setSinkId(deviceId);
}
