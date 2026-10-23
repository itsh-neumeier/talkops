import type { Call } from '../api.ts';

/** Inbound calls blocked by call blocking, outbound calls by a dialing profile. */
export function isBlocked(call: Pick<Call, 'direction' | 'blocked_reason'>): boolean {
	if (!call.blocked_reason) return false;
	if (call.direction === 'inbound') return true;
	return call.direction === 'outbound' && call.blocked_reason.startsWith('dialing:');
}

export function allowNumber(number: string): boolean {
	return /^\+[1-9][0-9]{2,14}$/.test(number);
}

export function blockSource(
	reason: string
): 'list' | 'phoneblock' | 'anonymous' | 'dialing' | 'unknown' {
	if (reason === 'anonymous') return 'anonymous';
	if (/^list:\+[0-9]{3,16}\*?$/.test(reason)) return 'list';
	if (/^phoneblock:[0-9]+$/.test(reason)) return 'phoneblock';
	if (reason.startsWith('dialing:')) return 'dialing';
	return 'unknown';
}
