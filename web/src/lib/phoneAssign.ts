import { api, type DeviceCredentials, type DeviceKind } from '#lib/api.ts';

/** Device type that matches a phone family. */
export function deviceKind(family: string | undefined): DeviceKind {
	if (family === 'dect') return 'dect';
	if (family === 'wifi') return 'wifi';
	return 'desk';
}

/**
 * Puts an extension on a provisioned phone: a new device of the extension,
 * placed on the next free account of the phone.
 */
export function assignExtension(
	phone: { id: string; name: string },
	family: string | undefined,
	extensionId: string
): Promise<DeviceCredentials> {
	return api.post<DeviceCredentials>(`/extensions/${extensionId}/devices`, {
		name: phone.name,
		kind: deviceKind(family),
		phone_id: phone.id,
		account_index: null
	});
}
