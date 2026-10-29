import type { PhoneParameter } from './api.ts';
export const parameterCategories = [
	'locale',
	'network',
	'sip',
	'calls',
	'audio',
	'display',
	'keys',
	'directory',
	'provisioning',
	'maintenance',
	'security',
	'other'
] as const;
export type ParameterCategory = (typeof parameterCategories)[number];
export function parameterCategory(name: string): ParameterCategory {
	const key = name.toLowerCase();
	const groups: [ParameterCategory, RegExp][] = [
		['security', /password|passwd|secret|certificate|tls|https|auth|vpn|srtp|private|encrypt/],
		['locale', /language|locale|country|time|ntp|daylight|(?:^|[_.])(?:dst|date)(?:$|[_.])/],
		[
			'network',
			/network|vlan|dhcp|dns|gateway|ip_address|netmask|ethernet|wifi|wireless|wlan|stun|qos/
		],
		['keys', /key|button|dss|blf|speed_dial/],
		['directory', /phonebook|directory|contact|ldap/],
		[
			'audio',
			/audio|codec|volume|ringer|ringtone|tone|headset|handset|speaker|microphone|echo|dtmf|noise/
		],
		['display', /display|screen|backlight|wallpaper|logo|lcd|brightness|contrast|font|color/],
		['provisioning', /provision|firmware|upgrade|update|download|config_server|tftp/],
		['calls', /forward|dnd|waiting|pickup|dial|transfer|conference|call|voicemail|intercom|paging/],
		['sip', /sip|account|register|proxy|voip|rtp|transport|expire/],
		['maintenance', /log|debug|reboot|reset|trace|snmp|web|admin/]
	];
	return groups.find(([, pattern]) => pattern.test(key))?.[0] ?? 'other';
}
export function visibleParameters(
	fields: PhoneParameter[],
	search: string,
	configuredOnly: boolean,
	values: Record<string, string> = {}
) {
	const query = search.trim().toLowerCase();
	return fields.filter(
		(field) =>
			(!configuredOnly || Object.hasOwn(values, field.key)) &&
			(!query ||
				`${field.parameter} ${field.description} ${field.artifact}`.toLowerCase().includes(query))
	);
}
/**
 * Readable label for an XML path parameter such as
 * `device[0]/devicePool[0]/dateTimeSetting[0]/dateTemplate[0]`: the last two
 * elements without indexes (`dateTimeSetting › dateTemplate`). Other
 * parameter names stay unchanged.
 */
export function parameterLabel(parameter: string): string {
	if (!parameter.includes('/')) return parameter;
	const parts = parameter
		.split('/')
		.map((part) => part.replace(/\[0\]$/, '').replace(/\[(\d+)\]$/, ' $1'))
		.filter(Boolean);
	return parts.slice(-2).join(' › ');
}
/** Delivered file name for an artifact pattern (`SEP{$mac|upper}.cnf.xml`). */
export function artifactFileName(artifact: string, mac?: string): string {
	if (!mac) return artifact;
	return artifact
		.replaceAll('{$mac|upper}', mac.toUpperCase())
		.replaceAll('{$mac|lower}', mac.toLowerCase())
		.replaceAll('{$mac}', mac)
		.replaceAll('{mac}', mac);
}
