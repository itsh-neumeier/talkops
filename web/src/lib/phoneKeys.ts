import type { Contact, LineKey, PhoneAccount, PhoneModel } from './api.ts';

/** The same effective layout as the provisioning renderer, without mutating overrides. */
export function effectivePhoneKey(n: number, keys: LineKey[], accounts: PhoneAccount[]): LineKey {
	const override = keys.find((k) => k.key === n);
	if (override) return override;
	const account = accounts.find((a) => a.account_index === n);
	return account
		? {
				key: n,
				type: 'line',
				value: '',
				label: account.phone_label || account.extension_number,
				account: n
			}
		: { key: n, type: 'none', value: '', label: '', account: 1 };
}

export function phoneKeyPages(model: PhoneModel): number[][] {
	return model.layout?.pages ?? [Array.from({ length: model.line_keys }, (_, i) => i + 1)];
}

export function unsupportedKey(key: LineKey, model: PhoneModel): boolean {
	return (
		key.key < 1 ||
		key.key > model.line_keys ||
		(key.type === 'line' && key.key > (model.line_key_limit ?? model.accounts))
	);
}

/** Phonebook assignment is a display filter, not an authorization boundary. */
export function contactsForPhone(
	contacts: Contact[],
	sectionIds: string[],
	book = '',
	search = ''
): Contact[] {
	const query = search.trim().toLocaleLowerCase();
	return contacts
		.filter(
			(c) =>
				(c.section_id === null || sectionIds.includes(c.section_id)) &&
				(!book || (book === 'global' ? c.section_id === null : c.section_id === book)) &&
				[c.name, c.company, c.phone_work, c.phone_mobile, c.phone_other].some((v) =>
					v.toLocaleLowerCase().includes(query)
				)
		)
		.sort((a, b) => a.name.localeCompare(b.name));
}

export function contactNumbers(
	contact: Contact
): { kind: 'work' | 'mobile' | 'other'; value: string }[] {
	return (['work', 'mobile', 'other'] as const).flatMap((kind) => {
		const value = contact[`phone_${kind}`];
		return value ? [{ kind, value }] : [];
	});
}

export function keyValidation(
	key: LineKey,
	model: PhoneModel,
	accounts: PhoneAccount[]
): string | null {
	if (unsupportedKey(key, model)) return 'phonekeys.invalidSlot';
	if (key.type !== 'none' && !accounts.some((a) => a.account_index === key.account))
		return 'phonekeys.missingAccount';
	if (Array.from(key.label).length > 32) return 'phonekeys.longLabel';
	if (key.type === 'speed_dial' && !model.speed_dial_keys) return 'phonekeys.noSpeedDial';
	if (key.type === 'speed_dial' || key.type === 'blf') {
		const park = key.type === 'blf' && /^park\+\*7[1-9]$/.test(key.value);
		if (!park && (!/^[0-9*#+]+$/.test(key.value) || key.value.length > 32))
			return 'phonekeys.invalidNumber';
	}
	return null;
}
