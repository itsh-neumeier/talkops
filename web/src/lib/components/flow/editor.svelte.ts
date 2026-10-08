// Shared state of the Smart Attendant editor: the flow being edited and the
// selected step (with the slot holding it, so it can be replaced).

import type { FlowNode, FlowNodeType } from '#lib/api.ts';
import { createNode, newId, walk } from '#lib/flow.ts';

export interface Slot {
	holder: Record<string, unknown>;
	key: string;
}

/** An entry of the right-click menu; `children` open a sub-list. */
export interface MenuItem {
	label: string;
	danger?: boolean;
	action?: () => void;
	children?: MenuItem[];
}

export class FlowEditor {
	/** `root.flow` is the first step (`null` while the flow is empty). */
	root = $state<{ flow: FlowNode | null }>({ flow: null });
	selected = $state<string | null>(null);
	slot = $state<Slot | null>(null);
	/** The empty slot whose "add step" menu is open. */
	adding = $state.raw<object | null>(null);
	/** The open right-click menu (viewport position). */
	menu = $state.raw<{ x: number; y: number; items: MenuItem[] } | null>(null);
	/** Set by the settings panel: saves pending audio before switching. */
	flush: (() => Promise<void>) | null = null;

	constructor(flow: FlowNode | null) {
		this.root.flow = flow;
	}

	get rootSlot(): Slot {
		return { holder: this.root as Record<string, unknown>, key: 'flow' };
	}

	async select(id: string | null, slot: Slot | null) {
		if (id === this.selected) return;
		try {
			await this.flush?.();
		} catch {
			// Nothing generated yet; the step keeps its previous audio.
		}
		this.selected = id;
		this.slot = slot;
	}

	/** The selected step (looked up again, since slots hold proxies). */
	get node(): FlowNode | null {
		const slot = this.slot;
		if (!slot) return null;
		const node = slot.holder[slot.key] as FlowNode | null;
		return node && node.id === this.selected ? node : null;
	}

	remove() {
		if (this.slot) this.removeAt(this.slot);
	}

	/** Removes the step in `slot` and everything below it. */
	removeAt(slot: Slot) {
		const node = slot.holder[slot.key] as FlowNode | null;
		if (node && walk(node).some((n) => n.id === this.selected)) {
			this.selected = null;
			this.slot = null;
		}
		slot.holder[slot.key] = null;
	}

	/** Replaces the step in `slot` by a new step of `type` (its branches go). */
	replaceAt(slot: Slot, type: FlowNodeType) {
		this.removeAt(slot);
		const root = this.root.flow;
		const id = newId(root, type);
		slot.holder[slot.key] = createNode(type, id, root?.id ?? id);
		void this.select(id, slot);
	}
}
