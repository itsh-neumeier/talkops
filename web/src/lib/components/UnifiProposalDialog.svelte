<script lang="ts">
	// Diff dialog for changes to the UniFi voice SSID (ADR 0042). Opening it
	// only reads the SSID. "Apply" writes after the confirmation check box.
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import {
		applyBody,
		initialSelection,
		slotText,
		valueText,
		warnings,
		writeErrorKey,
		type FieldKey,
		type Proposal,
		type WifiChange
	} from '#lib/unifiWrite.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		open = $bindable(false),
		surveyId,
		deviceId
	}: { open?: boolean; surveyId: string; deviceId: string | null } = $props();

	let proposal = $state<Proposal | null>(null);
	let selected = $state<Set<FieldKey>>(new Set());
	let confirmed = $state(false);
	let busy = $state(false);
	let error = $state('');
	let applied = $state<WifiChange | null>(null);

	const labels = $derived({
		on: t('unifiWrite.value.on'),
		off: t('unifiWrite.value.off'),
		absent: t('unifiWrite.value.absent')
	});

	function message(err: unknown): string {
		const key = writeErrorKey(err);
		return key ? t(key as MessageKey) : errorMessage(err);
	}

	async function load() {
		proposal = null;
		applied = null;
		confirmed = false;
		error = '';
		try {
			proposal = await api.get<Proposal>(`/wifi-surveys/${surveyId}/unifi-proposal`);
			selected = initialSelection(proposal);
		} catch (err) {
			error = message(err);
		}
	}

	$effect(() => {
		if (open) load();
	});

	function toggle(field: FieldKey) {
		const next = new Set(selected);
		if (next.has(field)) next.delete(field);
		else next.add(field);
		selected = next;
	}

	async function apply() {
		if (!proposal) return;
		busy = true;
		error = '';
		try {
			applied = await api.post<WifiChange>(
				'/unifi/wifi-changes',
				applyBody(proposal, selected, surveyId)
			);
		} catch (err) {
			error = message(err);
		} finally {
			busy = false;
		}
	}

	const blocked = $derived(
		proposal?.not_writable
			? `unifiWrite.err.${proposal.not_writable}`
			: proposal && !proposal.write.allowed && proposal.write.reason
				? `unifiWrite.err.${proposal.write.reason}`
				: null
	);
	const canApply = $derived(
		!!proposal && !blocked && confirmed && selected.size > 0 && !busy && !applied
	);
</script>

<Modal title={t('unifiWrite.dialogTitle')} bind:open>
	<div class="space-y-3 text-sm">
		<ErrorBox {error} />
		{#if !proposal && !error}
			<p class="text-slate-500 dark:text-slate-400" role="status">{t('unifiWrite.loading')}</p>
		{:else if proposal && applied}
			<p class="text-teal-700 dark:text-teal-400" role="status">{t('unifiWrite.applied')}</p>
			<div class="flex flex-wrap gap-2">
				{#if deviceId}
					<a class="btn btn-primary" href={`/wifi-surveys?start=${deviceId}`}
						>{t('unifiWrite.newSurvey')}</a
					>
				{/if}
				<a class="btn" href="/wifi-surveys?tab=changes">{t('unifiWrite.openHistory')}</a>
			</div>
		{:else if proposal}
			<p class="font-medium">{t('unifiWrite.ssid', { name: proposal.wifi_name })}</p>
			{#if proposal.changes.length}
				<div class="scroll-x">
					<table class="table">
						<thead>
							<tr>
								<th class="w-8"><span class="sr-only">{t('unifiWrite.apply')}</span></th>
								<th>{t('unifiWrite.col.field')}</th>
								<th>{t('unifiWrite.col.current')}</th>
								<th>{t('unifiWrite.col.new')}</th>
							</tr>
						</thead>
						<tbody>
							{#each proposal.changes as c (c.field)}
								{@const name = t(`unifiWrite.field.${c.field}` as MessageKey)}
								<tr>
									<td>
										<input
											type="checkbox"
											aria-label={t('unifiWrite.select', { field: name })}
											checked={selected.has(c.field)}
											disabled={!!applied}
											onchange={() => toggle(c.field)}
										/>
									</td>
									<td>
										<div>{name}</div>
										<div class="text-xs text-slate-500 dark:text-slate-400">
											{t(`unifiWrite.rec.${c.recommendation}` as MessageKey)}
										</div>
									</td>
									<td class="text-slate-500 dark:text-slate-400">
										{slotText(c.field, c.current, labels)}
									</td>
									<td class="font-medium">{valueText(c.field, c.proposed, labels)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else}
				<p class="text-slate-600 dark:text-slate-300">{t('unifiWrite.noChanges')}</p>
			{/if}

			{#if proposal.hints.length}
				<div class="space-y-1">
					<p class="text-slate-600 dark:text-slate-300">{t('unifiWrite.hintsTitle')}</p>
					<ul class="list-disc space-y-1 pl-5 text-slate-600 dark:text-slate-300">
						{#each proposal.hints as h (h.id)}
							<li>{t(`unifiWrite.hint.${h.id}` as MessageKey)}</li>
						{/each}
					</ul>
				</div>
			{/if}

			{#if proposal.changes.length}
				{#each warnings(proposal, selected) as w (w)}
					<p
						class="rounded-lg bg-amber-50 p-2 text-amber-800 dark:bg-amber-950/40 dark:text-amber-300"
					>
						{t(`unifiWrite.warn.${w}` as MessageKey)}
					</p>
				{/each}
				{#if blocked}
					<p class="error-box">{t(blocked as MessageKey)}</p>
				{:else}
					<label class="flex items-start gap-2">
						<input type="checkbox" class="mt-0.5" bind:checked={confirmed} />
						<span>{t('unifiWrite.confirm')}</span>
					</label>
				{/if}
				<div class="flex justify-end gap-2">
					<button class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
					<button class="btn btn-primary" disabled={!canApply} onclick={apply}
						>{t('unifiWrite.apply')}</button
					>
				</div>
			{/if}
		{/if}
	</div>
</Modal>
