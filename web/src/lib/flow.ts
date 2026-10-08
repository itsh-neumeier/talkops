// Smart Attendant flows: creating steps, walking the tree, labels and icons.

import type { FlowNode, FlowNodeType } from '#lib/api.ts';
import type { MessageKey } from '#lib/i18n/index.svelte.ts';

/** Step types in the order of the "add step" menu. */
export const NODE_TYPES: FlowNodeType[] = [
	'menu',
	'ring',
	'play',
	'schedule',
	'voicemail',
	'transfer',
	'park',
	'goto',
	'hangup'
];

export const typeKey = (type: FlowNodeType) => `flow.type.${type}` as MessageKey;
export const typeHint = (type: FlowNodeType) => `flow.hint.${type}` as MessageKey;

/** 24×24 outline icon paths (stroke) per step type. */
export const ICONS: Record<FlowNodeType, string> = {
	menu: 'M5 4h4v4H5zM10 4h4v4h-4zM15 4h4v4h-4zM5 10h4v4H5zM10 10h4v4h-4zM15 10h4v4h-4zM10 16h4v4h-4z',
	ring: 'M5 4h4l2 5-2.5 1.5a11 11 0 0 0 5 5L15 13l5 2v4a2 2 0 0 1-2 2A16 16 0 0 1 3 6a2 2 0 0 1 2-2',
	play: 'M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14',
	schedule: 'M12 7v5l3 3M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0',
	voicemail: 'M6 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6M18 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6M6 15h12',
	transfer: 'M4 12h14M13 6l6 6-6 6',
	park: 'M7 21V3h6a5 5 0 0 1 0 10H7',
	goto: 'M9 14 4 9l5-5M4 9h11a5 5 0 0 1 0 10h-3',
	hangup: 'M18 6 6 18M6 6l12 12'
};

/** Steps that end the call or leave the flow (no step after them). */
export const isTerminal = (type: FlowNodeType) =>
	['voicemail', 'transfer', 'park', 'goto', 'hangup'].includes(type);

export function walk(node: FlowNode | null): FlowNode[] {
	if (!node) return [];
	const below: (FlowNode | null)[] = [];
	switch (node.type) {
		case 'play':
		case 'ring':
			below.push(node.next);
			break;
		case 'menu':
			below.push(...node.options.map((o) => o.next), node.timeout);
			break;
		case 'schedule':
			below.push(node.open, node.closed);
			break;
	}
	return [node, ...below.flatMap(walk)];
}

/** A step id not used in `root` yet. */
export function newId(root: FlowNode | null, type: FlowNodeType): string {
	const used = new Set(walk(root).map((n) => n.id));
	for (let i = 1; ; i++) {
		const id = `${type}-${i}`;
		if (!used.has(id)) return id;
	}
}

/** A new step with sensible defaults. */
export function createNode(type: FlowNodeType, id: string, firstStep: string): FlowNode {
	switch (type) {
		case 'play':
			return { type, id, clip_id: null, next: null };
		case 'menu':
			return {
				type,
				id,
				clip_id: null,
				timeout_secs: 5,
				max_tries: 3,
				direct_dial: false,
				options: [],
				timeout: null
			};
		case 'ring':
			return { type, id, extensions: [], strategy: 'simultaneous', ring_secs: 30, next: null };
		case 'schedule':
			return { type, id, time_condition_id: null, open: null, closed: null };
		case 'voicemail':
			return { type, id, recipients: [], clip_id: null, max_message_secs: 120 };
		case 'transfer':
			return { type, id, destination_type: 'none', destination_id: null };
		case 'goto':
			return { type, id, target: firstStep };
		case 'park':
			return { type, id };
		case 'hangup':
			return { type, id };
	}
}

export const DIGITS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '*', '#'];
