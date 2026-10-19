import type { Call } from '../api.ts';

export function isBlocked(call: Pick<Call, 'direction' | 'blocked_reason'>): boolean {
	return call.direction === 'inbound' && !!call.blocked_reason;
}

export function allowNumber(number: string): boolean {
	return /^\+[1-9][0-9]{2,14}$/.test(number);
}

export function blockSource(reason: string): 'list' | 'phoneblock' | 'anonymous' | 'unknown' {
	if (reason === 'anonymous') return 'anonymous';
	if (/^list:\+[0-9]{3,16}\*?$/.test(reason)) return 'list';
	if (/^phoneblock:[0-9]+$/.test(reason)) return 'phoneblock';
	return 'unknown';
}
