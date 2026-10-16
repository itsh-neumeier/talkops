import type { KeyType, PhoneProvisioning, ProvisioningKey } from './api.ts';

export function keyNeedsAccount(type: KeyType): boolean {
	return ['line', 'blf', 'speed_dial', 'pickup', 'voicemail', 'dtmf', 'intercom'].includes(type);
}
export function keyNeedsNumber(type: KeyType): boolean {
	return ['blf', 'speed_dial', 'pickup', 'dtmf', 'intercom', 'prefix'].includes(type);
}
export function putProvisioningKey(
	values: PhoneProvisioning,
	key: ProvisioningKey | null,
	group: string,
	slot: number
): PhoneProvisioning {
	const keys = (values.keys ?? []).filter((k) => k.group !== group || k.key !== slot);
	if (key) keys.push({ ...key });
	return { ...values, keys };
}
export function configurationDiff(before: string, after: string): string {
	const old = new Set(before.split('\n')),
		next = new Set(after.split('\n'));
	return [...old]
		.filter((line) => !next.has(line))
		.map((line) => '- ' + line)
		.concat([...next].filter((line) => !old.has(line)).map((line) => '+ ' + line))
		.join('\n');
}
