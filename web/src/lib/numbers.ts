import { api, type PhoneNumber } from '#lib/api.ts';

/** Saves a phone number with changes; always sends every field. */
export function saveNumber(n: PhoneNumber, patch: Partial<PhoneNumber> = {}): Promise<PhoneNumber> {
	const next = { ...n, ...patch };
	return api.put<PhoneNumber>(`/numbers/${n.id}`, {
		trunk_id: next.trunk_id,
		account_id: next.account_id,
		e164: next.e164,
		label: next.label,
		destination_type: next.destination_type,
		destination_id: next.destination_id,
		extra_extensions: next.destination_type === 'extension' ? next.extra_extensions : [],
		enabled: next.enabled
	});
}

/** Does the number ring this extension (as main or additional extension)? */
export function ringsExtension(n: PhoneNumber, extensionId: string): boolean {
	return (
		n.destination_type === 'extension' &&
		(n.destination_id === extensionId || n.extra_extensions.includes(extensionId))
	);
}
