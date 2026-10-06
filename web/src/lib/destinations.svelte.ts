// Everything a call can be sent to, loaded once per page for the
// destination pickers (numbers, fallbacks, menu keys, schedules).

import {
	api,
	type DestinationType,
	type Extension,
	type IvrMenu,
	type Queue,
	type RingGroup,
	type TimeCondition
} from './api.ts';
import { t } from './i18n/index.svelte.ts';

export const targets = $state({
	extensions: [] as Extension[],
	groups: [] as RingGroup[],
	conditions: [] as TimeCondition[],
	menus: [] as IvrMenu[],
	queues: [] as Queue[]
});

export async function loadTargets() {
	const [extensions, groups, conditions, menus, queues] = await Promise.all([
		api.get<Extension[]>('/extensions'),
		api.get<RingGroup[]>('/ring-groups'),
		api.get<TimeCondition[]>('/time-conditions'),
		api.get<IvrMenu[]>('/ivr-menus'),
		api.get<Queue[]>('/queues')
	]);
	Object.assign(targets, { extensions, groups, conditions, menus, queues });
}

export interface TargetOption {
	type: DestinationType;
	id: string;
	label: string;
}

const named = (n: { number: string | null; name: string }) =>
	n.number ? `${n.number} ${n.name}` : n.name;

/** Options grouped by destination type (labels are i18n keys). */
export function targetGroups(): { key: string; options: TargetOption[] }[] {
	return [
		{
			key: 'dest.extension',
			options: targets.extensions.map((e) => ({
				type: 'extension',
				id: e.id,
				label: `${e.number} ${e.display_name}`
			}))
		},
		{
			key: 'dest.voicemail',
			options: targets.extensions.map((e) => ({
				type: 'voicemail',
				id: e.id,
				label: `${t('dest.voicemail')}: ${e.number} ${e.display_name}`
			}))
		},
		{
			key: 'dest.ring_group',
			options: targets.groups.map((g) => ({ type: 'ring_group', id: g.id, label: named(g) }))
		},
		{
			key: 'dest.time_condition',
			options: targets.conditions.map((c) => ({
				type: 'time_condition',
				id: c.id,
				label: named(c)
			}))
		},
		{
			key: 'dest.ivr',
			options: targets.menus.map((m) => ({ type: 'ivr', id: m.id, label: named(m) }))
		},
		{
			key: 'dest.queue',
			options: targets.queues.map((q) => ({ type: 'queue', id: q.id, label: named(q) }))
		}
	].filter((g) => g.options.length > 0) as { key: string; options: TargetOption[] }[];
}

/** Human-readable destination, e.g. "Ring group: 50 Support". */
export function describe(type: DestinationType, id: string | null): TargetOption | null {
	if (type === 'none' || !id) return null;
	for (const g of targetGroups()) {
		const hit = g.options.find((o) => o.type === type && o.id === id);
		if (hit) return hit;
	}
	return null;
}
