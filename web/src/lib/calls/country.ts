import { parsePhoneNumberFromString, type CountryCode } from 'libphonenumber-js/max';

/** Resolve international numbers only. Tenant normalization belongs to the API. */
export function numberCountry(number: string, internal = false): CountryCode | null {
	if (internal || number.length > 64) return null;
	const compact = number
		.trim()
		.replace(/[\s()./-]/g, '')
		.replace(/^00/, '+');
	if (!/^\+[1-9]\d{5,14}$/.test(compact)) return null;
	const parsed = parsePhoneNumberFromString(compact, { extract: false });
	if (!parsed?.isPossible() || !parsed.country) return null;
	// Shared service ranges (for example NANP toll-free) do not identify a country.
	if (
		['TOLL_FREE', 'PREMIUM_RATE', 'SHARED_COST', 'UAN', 'VOICEMAIL'].includes(
			parsed.getType() ?? ''
		)
	)
		return null;
	return parsed.country;
}
