<script lang="ts">
	// Settings of the selected step (sidebar of the Smart Attendant editor).
	import type { FlowNode } from '#lib/api.ts';
	import AudioPicker, { type AudioMode } from '#lib/components/AudioPicker.svelte';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import { targets } from '#lib/destinations.svelte.ts';
	import { DIGITS, ICONS, typeHint, typeKey, walk } from '#lib/flow.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import ExtensionPicker from '#lib/components/ExtensionPicker.svelte';
	import type { FlowEditor } from './editor.svelte.ts';

	let {
		editor,
		attendantId,
		language
	}: { editor: FlowEditor; attendantId: string; language: 'de' | 'en' } = $props();

	const ids = $props.id();
	const node = $derived(editor.node);
	let picker = $state<ReturnType<typeof AudioPicker>>();
	let mode = $state<AudioMode>('generate');

	// A fresh audio mode for each step; the picker corrects it from the clip.
	let modeFor = '';
	$effect.pre(() => {
		const n = node;
		if (!n || n.id === modeFor) return;
		modeFor = n.id;
		mode = n.type === 'voicemail' && !n.clip_id ? 'default' : 'generate';
	});

	// Before switching steps or saving: generate pending text, keep uploads.
	$effect(() => {
		editor.flush = async () => {
			const n = editor.node;
			if (!n || !picker || !('clip_id' in n)) return;
			n.clip_id = await picker.ensure();
		};
		return () => {
			editor.flush = null;
		};
	});

	const gotoTargets = $derived(
		walk(editor.root.flow).filter((n) => n.type !== 'goto' && n.id !== node?.id)
	);

	function freeDigits(n: Extract<FlowNode, { type: 'menu' }>, current: string) {
		return DIGITS.filter(
			(d) =>
				d === current || (!n.options.some((o) => o.digit === d) && !(d === '#' && n.direct_dial))
		);
	}

	const isRoot = $derived(editor.root.flow?.id === node?.id);
</script>

{#if node}
	<div class="space-y-4">
		<div class="flex items-start gap-3">
			<span
				class="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-teal-600/10 text-teal-700 dark:text-teal-300"
			>
				<svg
					viewBox="0 0 24 24"
					class="h-6 w-6"
					fill="none"
					stroke="currentColor"
					stroke-width="1.8"
					stroke-linecap="round"
					stroke-linejoin="round"><path d={ICONS[node.type]} /></svg
				>
			</span>
			<div class="min-w-0">
				<h2 class="text-base">{t(typeKey(node.type))}</h2>
				<p class="hint">{t(typeHint(node.type))}</p>
				<p class="font-mono text-xs text-slate-400">{node.id}</p>
			</div>
		</div>

		{#if node.type === 'play' || node.type === 'menu' || node.type === 'voicemail'}
			<fieldset class="space-y-2">
				<legend class="text-sm font-medium"
					>{node.type === 'voicemail' ? t('vm.greeting') : t('flow.audio')}</legend
				>
				{#key node.id}
					<AudioPicker
						bind:this={picker}
						bind:mode
						bind:clipId={node.clip_id}
						modes={node.type === 'voicemail'
							? ['default', 'generate', 'record', 'upload']
							: ['generate', 'record', 'upload']}
						{language}
						optional
						hints={{ default: t('flow.vmDefaultGreeting') }}
					/>
				{/key}
			</fieldset>
		{/if}

		{#if node.type === 'menu'}
			<fieldset class="space-y-2">
				<legend class="text-sm font-medium">{t('ivr.options')}</legend>
				{#each node.options as o, i (i)}
					<div class="flex items-center gap-2">
						<select class="input w-20 font-mono" bind:value={o.digit} aria-label={t('flow.key')}>
							{#each freeDigits(node, o.digit) as d (d)}<option value={d}>{d}</option>{/each}
						</select>
						<span class="flex-1 truncate text-sm text-slate-500"
							>→ {o.next ? t(typeKey(o.next.type)) : t('flow.end.hangup')}</span
						>
						<button
							type="button"
							class="btn btn-sm"
							onclick={() => {
								if (node.type === 'menu') node.options.splice(i, 1);
							}}
							aria-label={t('common.delete')}>✕</button
						>
					</div>
				{:else}
					<p class="hint">{t('flow.noKeys')}</p>
				{/each}
			</fieldset>
			<div class="grid grid-cols-2 gap-3">
				<div>
					<label for="{ids}-to">{t('ivr.timeout')}</label>
					<input
						id="{ids}-to"
						class="input"
						type="number"
						min="1"
						max="30"
						bind:value={node.timeout_secs}
					/>
				</div>
				<div>
					<label for="{ids}-tries">{t('ivr.tries')}</label>
					<input
						id="{ids}-tries"
						class="input"
						type="number"
						min="1"
						max="10"
						bind:value={node.max_tries}
					/>
				</div>
			</div>
			<label class="flex items-center gap-2">
				<input type="checkbox" bind:checked={node.direct_dial} />
				{t('ivr.directDial')}
			</label>
		{:else if node.type === 'ring'}
			<ExtensionPicker bind:selected={node.extensions} label={t('flow.ringWho')} />
			<div class="grid grid-cols-2 gap-3">
				<div>
					<label for="{ids}-strategy">{t('flow.strategy')}</label>
					<select id="{ids}-strategy" class="input" bind:value={node.strategy}>
						<option value="simultaneous">{t('flow.simultaneous')}</option>
						<option value="sequential">{t('flow.sequential')}</option>
					</select>
				</div>
				<div>
					<label for="{ids}-secs">{t('flow.ringSecs')}</label>
					<input
						id="{ids}-secs"
						class="input"
						type="number"
						min="5"
						max="300"
						bind:value={node.ring_secs}
					/>
				</div>
			</div>
			<p class="hint">{t('flow.ringHint')}</p>
		{:else if node.type === 'schedule'}
			<div>
				<label for="{ids}-tc">{t('flow.schedule')}</label>
				<select id="{ids}-tc" class="input" bind:value={node.time_condition_id}>
					<option value={null}>—</option>
					{#each targets.conditions as c (c.id)}
						<option value={c.id}>{c.number ? `${c.number} ` : ''}{c.name}</option>
					{/each}
				</select>
			</div>
			{@const tc = targets.conditions.find((c) => c.id === node.time_condition_id)}
			{#if tc}
				<p class="text-sm">
					{t('flow.scheduleNow')}
					<span class={tc.state.open ? 'text-emerald-700 dark:text-emerald-400' : 'text-red-600'}
						>{tc.state.open ? t('flow.branch.open') : t('flow.branch.closed')}</span
					>
				</p>
			{/if}
			<p class="hint">{t('flow.scheduleHint')}</p>
		{:else if node.type === 'voicemail'}
			<ExtensionPicker bind:selected={node.recipients} label={t('flow.recipients')} />
			<p class="hint">{t('flow.recipientsHint')}</p>
			<div>
				<label for="{ids}-max">{t('vm.maxLength')}</label>
				<input
					id="{ids}-max"
					class="input"
					type="number"
					min="10"
					max="600"
					bind:value={node.max_message_secs}
				/>
			</div>
		{:else if node.type === 'transfer'}
			<div>
				<label for="{ids}-dest">{t('flow.destination')}</label>
				<DestinationSelect
					inputId="{ids}-dest"
					bind:type={node.destination_type}
					bind:id={node.destination_id}
					exclude={attendantId}
				/>
			</div>
		{:else if node.type === 'goto'}
			<div>
				<label for="{ids}-goto">{t('flow.gotoTarget')}</label>
				<select id="{ids}-goto" class="input" bind:value={node.target}>
					{#each gotoTargets as n (n.id)}
						<option value={n.id}>{t(typeKey(n.type))} ({n.id})</option>
					{/each}
				</select>
			</div>
		{/if}

		<div class="border-t border-slate-200 pt-3 dark:border-slate-700">
			<button type="button" class="btn btn-sm btn-danger" onclick={() => editor.remove()}>
				{isRoot ? t('flow.removeAll') : t('flow.remove')}
			</button>
		</div>
	</div>
{:else}
	<p class="text-sm text-slate-500">{t('flow.selectHint')}</p>
{/if}
