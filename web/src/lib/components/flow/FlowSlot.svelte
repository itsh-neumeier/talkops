<script lang="ts">
	// One place in the flow: either a step (card plus the steps below it) or
	// an empty slot with a "+" to add a step. Renders itself recursively.
	import type { FlowNode, FlowNodeType } from '#lib/api.ts';
	import { describe, targets } from '#lib/destinations.svelte.ts';
	import { ICONS, NODE_TYPES, DIGITS, createNode, newId, typeHint, typeKey } from '#lib/flow.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import Self from './FlowSlot.svelte';
	import type { FlowEditor } from './editor.svelte.ts';

	let {
		holder,
		key,
		editor,
		branch = '',
		ending = ''
	}: {
		holder: Record<string, unknown>;
		key: string;
		editor: FlowEditor;
		/** Label of the branch leading here ("Key 1", "Open" …). */
		branch?: string;
		/** What happens when this slot stays empty. */
		ending?: string;
	} = $props();

	const node = $derived(holder[key] as FlowNode | null);
	// Identifies this slot's "add step" menu (only one is open at a time).
	const menuKey = {};
	const adding = $derived(editor.adding === menuKey);

	$effect(() => {
		if (!adding) return;
		const close = (e: Event) => {
			if (e instanceof KeyboardEvent && e.key !== 'Escape') return;
			// "+" buttons and menus handle their own clicks.
			if (e.target instanceof Element && e.target.closest('[data-flow-add]')) return;
			editor.adding = null;
		};
		// Defer so the opening click does not close the menu right away.
		const timer = setTimeout(() => {
			window.addEventListener('click', close);
			window.addEventListener('keydown', close);
		});
		return () => {
			clearTimeout(timer);
			window.removeEventListener('click', close);
			window.removeEventListener('keydown', close);
		};
	});

	function add(type: FlowNodeType) {
		const root = editor.root.flow;
		const id = newId(root, type);
		holder[key] = createNode(type, id, root?.id ?? id);
		editor.adding = null;
		void editor.select(id, { holder, key });
	}

	const extName = (id: string) => {
		const e = targets.extensions.find((x) => x.id === id);
		return e ? `${e.number} ${e.display_name}` : '?';
	};

	function summary(n: FlowNode): string {
		switch (n.type) {
			case 'play':
			case 'menu':
				return n.clip_id ? t('flow.sum.audio') : t('flow.sum.noAudio');
			case 'ring':
				return n.extensions.length
					? `${n.extensions.map(extName).join(', ')} · ${n.ring_secs} s`
					: t('flow.sum.nobody');
			case 'voicemail':
				return n.recipients.length ? n.recipients.map(extName).join(', ') : t('flow.sum.nobody');
			case 'schedule': {
				const tc = targets.conditions.find((c) => c.id === n.time_condition_id);
				return tc ? tc.name : t('flow.sum.noSchedule');
			}
			case 'transfer':
				return describe(n.destination_type, n.destination_id)?.label ?? t('flow.sum.noTarget');
			case 'goto':
				return t('flow.sum.goto', { step: n.target });
			default:
				return '';
		}
	}

	interface Branch {
		holder: Record<string, unknown>;
		key: string;
		label: string;
		ending: string;
	}

	const branches = $derived.by((): Branch[] => {
		if (!node) return [];
		const hangup = t('flow.end.hangup');
		switch (node.type) {
			case 'play':
				return [{ holder: node, key: 'next', label: '', ending: hangup }];
			case 'ring':
				return [{ holder: node, key: 'next', label: t('flow.branch.noAnswer'), ending: hangup }];
			case 'menu':
				return [
					...node.options.map((o) => ({
						holder: o,
						key: 'next',
						label: t('flow.branch.key', { key: o.digit }),
						ending: hangup
					})),
					{ holder: node, key: 'timeout', label: t('flow.branch.noInput'), ending: hangup }
				];
			case 'schedule':
				return [
					{ holder: node, key: 'open', label: t('flow.branch.open'), ending: hangup },
					{ holder: node, key: 'closed', label: t('flow.branch.closed'), ending: hangup }
				];
			default:
				return [];
		}
	});

	function addKey() {
		if (node?.type !== 'menu') return;
		const free = DIGITS.find(
			(d) => !node.options.some((o) => o.digit === d) && !(d === '#' && node.direct_dial)
		);
		if (free) node.options.push({ digit: free, next: null });
	}

	const selected = $derived(node !== null && editor.selected === node.id);
</script>

<div class="flex flex-col items-center">
	{#if branch}
		<span
			class="mb-1 rounded-full border border-slate-300 bg-white px-2 py-0.5 text-xs whitespace-nowrap text-slate-600 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-300"
			>{branch}</span
		>
	{/if}
	{#if node}
		<button
			type="button"
			class="w-60 rounded-xl border bg-white p-3 text-left shadow-sm transition dark:bg-slate-900 {selected
				? 'border-teal-600 ring-2 ring-teal-600/30'
				: 'border-slate-200 hover:border-teal-500 dark:border-slate-700'}"
			onclick={() => editor.select(node.id, { holder, key })}
			data-step={node.id}
		>
			<span class="flex items-center gap-2">
				<span
					class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-teal-600/10 text-teal-700 dark:text-teal-300"
				>
					<svg
						viewBox="0 0 24 24"
						class="h-5 w-5"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"><path d={ICONS[node.type]} /></svg
					>
				</span>
				<span class="min-w-0">
					<span class="block text-sm font-medium">{t(typeKey(node.type))}</span>
					<span class="block truncate text-xs text-slate-500">{summary(node) || ' '}</span>
				</span>
			</span>
		</button>
		{#if branches.length === 1}
			<div class="h-5 w-px bg-slate-300 dark:bg-slate-600"></div>
			<Self
				{editor}
				holder={branches[0].holder}
				key={branches[0].key}
				branch={branches[0].label}
				ending={branches[0].ending}
			/>
		{:else if branches.length > 1}
			<div class="h-5 w-px bg-slate-300 dark:bg-slate-600"></div>
			<div class="flex">
				{#each branches as b, i (i)}
					<div class="relative flex flex-col items-center px-3 pt-5">
						<span
							class="absolute top-0 h-px bg-slate-300 dark:bg-slate-600 {i === 0
								? 'right-0 left-1/2'
								: i === branches.length - 1 && !(node.type === 'menu')
									? 'right-1/2 left-0'
									: 'right-0 left-0'}"
						></span>
						<span class="absolute top-0 left-1/2 h-5 w-px bg-slate-300 dark:bg-slate-600"></span>
						<Self {editor} holder={b.holder} key={b.key} branch={b.label} ending={b.ending} />
					</div>
				{/each}
				{#if node.type === 'menu'}
					<div class="relative flex flex-col items-center px-3 pt-5">
						<span class="absolute top-0 right-1/2 left-0 h-px bg-slate-300 dark:bg-slate-600"
						></span>
						<span class="absolute top-0 left-1/2 h-5 w-px bg-slate-300 dark:bg-slate-600"></span>
						<button type="button" class="btn btn-sm whitespace-nowrap" onclick={addKey}
							>+ {t('flow.addKey')}</button
						>
					</div>
				{/if}
			</div>
		{/if}
	{:else}
		<div class="relative">
			<button
				type="button"
				class="inline-flex h-8 w-8 items-center justify-center rounded-full border border-dashed border-slate-400 text-slate-500 hover:border-teal-600 hover:text-teal-700 dark:border-slate-500"
				data-flow-add
				onclick={() => (editor.adding = adding ? null : menuKey)}
				aria-label={t('flow.add')}
				aria-expanded={adding}>+</button
			>
			{#if adding}
				<div
					class="absolute top-10 left-1/2 z-20 w-64 -translate-x-1/2 rounded-xl border border-slate-200 bg-white p-1 shadow-lg dark:border-slate-700 dark:bg-slate-900"
					role="menu"
					data-flow-add
				>
					{#each NODE_TYPES as type (type)}
						<button
							type="button"
							role="menuitem"
							class="flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left hover:bg-slate-100 dark:hover:bg-slate-800"
							onclick={() => add(type)}
						>
							<svg
								viewBox="0 0 24 24"
								class="mt-0.5 h-4 w-4 shrink-0 text-teal-700 dark:text-teal-300"
								fill="none"
								stroke="currentColor"
								stroke-width="1.8"
								stroke-linecap="round"
								stroke-linejoin="round"><path d={ICONS[type]} /></svg
							>
							<span>
								<span class="block text-sm">{t(typeKey(type))}</span>
								<span class="block text-xs text-slate-500">{t(typeHint(type))}</span>
							</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
		{#if ending}<span class="mt-1 text-xs text-slate-400">{ending}</span>{/if}
	{/if}
</div>
