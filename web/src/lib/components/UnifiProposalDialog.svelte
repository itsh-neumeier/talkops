<script lang="ts">
	// Diff dialog for changes to the UniFi voice SSID (ADR 0042) and to the
	// roaming threshold of the survey handset (ADR 0043). Opening it only
	// reads. "Apply" writes after the confirmation check box. A band removal
	// shows the band check of the Wi-Fi handsets first.
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import {
		applyBody,
		bandState,
		handsetBody,
		HANDSET_FIELD,
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
	let handsetSelected = $state(false);
	let unknownOk = $state<Set<string>>(new Set());
	let confirmed = $state(false);
	let busy = $state(false);
	let error = $state('');
	let ssidDone = $state<WifiChange | null>(null);
	let handsetDone = $state<WifiChange | null>(null);

	const labels = $derived({
		on: t('unifiWrite.value.on'),
		off: t('unifiWrite.value.off'),
		absent: t('unifiWrite.value.absent')
	});

	function message(err: unknown): string {
		const key = writeErrorKey(err);
		return key ? t(key as MessageKey) : errorMessage(err);
	}

	// Why TalkOps cannot write the SSID now, if it cannot.
	const blockedOf = (p: Proposal) =>
		p.not_writable
			? `unifiWrite.err.${p.not_writable}`
			: !p.write.allowed && p.write.reason
				? `unifiWrite.err.${p.write.reason}`
				: null;

	async function load() {
		proposal = null;
		ssidDone = null;
		handsetDone = null;
		confirmed = false;
		unknownOk = new Set();
		error = '';
		try {
			const p = await api.get<Proposal>(`/wifi-surveys/${surveyId}/unifi-proposal`);
			// A blocked SSID starts without rows, so the handset part can still apply.
			selected = blockedOf(p) ? new Set() : initialSelection(p);
			handsetSelected = !!p.handset?.write.allowed;
			proposal = p;
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

	function toggleUnknown(id: string) {
		const next = new Set(unknownOk);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		unknownOk = next;
	}

	async function apply() {
		if (!proposal) return;
		busy = true;
		error = '';
		try {
			if (selected.size && !ssidDone) {
				ssidDone = await api.post<WifiChange>(
					'/unifi/wifi-changes',
					applyBody(proposal, selected, surveyId, [...unknownOk])
				);
			}
			if (handsetSelected && proposal.handset && !handsetDone) {
				handsetDone = await api.post<WifiChange>(
					'/wifi-handset-changes',
					handsetBody(proposal.handset, surveyId)
				);
			}
		} catch (err) {
			error = message(err);
		} finally {
			busy = false;
		}
	}

	const blocked = $derived(proposal ? blockedOf(proposal) : null);
	const bands = $derived(
		proposal ? bandState(proposal, selected, unknownOk) : { blocked: false, unconfirmed: [] }
	);
	const handset = $derived(proposal?.handset ?? null);
	const anything = $derived(selected.size > 0 || (handsetSelected && !!handset));
	const done = $derived(!!ssidDone || !!handsetDone);
	const finished = $derived(
		done && (!selected.size || !!ssidDone) && (!handsetSelected || !handset || !!handsetDone)
	);
	const canApply = $derived(
		!!proposal &&
			anything &&
			!(selected.size && (blocked || bands.blocked || bands.unconfirmed.length)) &&
			confirmed &&
			!busy &&
			!finished
	);
</script>

<Modal title={t('unifiWrite.dialogTitle')} bind:open>
	<div class="space-y-3 text-sm">
		<ErrorBox {error} />
		{#if !proposal && !error}
			<p class="text-slate-500 dark:text-slate-400" role="status">{t('unifiWrite.loading')}</p>
		{:else if proposal && finished}
			{#if ssidDone}
				<p class="text-teal-700 dark:text-teal-400" role="status">{t('unifiWrite.applied')}</p>
			{/if}
			{#if handsetDone}
				<p class="text-teal-700 dark:text-teal-400" role="status">
					{t('unifiWrite.handsetWaiting', { name: handsetDone.phone_name })}
				</p>
			{/if}
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
											disabled={!!blocked || !!ssidDone}
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
			{#if blocked && proposal.changes.length}
				<p class="error-box">{t(blocked as MessageKey)}</p>
			{/if}

			{#if selected.has('broadcastingFrequenciesGHz') && proposal.band_check}
				<div class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
					<p class="font-medium">{t('unifiWrite.bandCheck')}</p>
					{#if !proposal.band_check.length}
						<p class="text-slate-500 dark:text-slate-400">{t('unifiWrite.bandNoHandsets')}</p>
					{/if}
					<ul class="space-y-1">
						{#each proposal.band_check as h (h.phone_id)}
							<li class="flex flex-wrap items-center gap-2">
								<span
									class="badge {h.state === 'ok'
										? 'badge-ok'
										: h.state === 'blocked'
											? 'badge-bad'
											: 'badge-warn'}">{t(`unifiWrite.band.${h.state}`)}</span
								>
								<span>{h.name}{h.band != null ? ` · ${h.band} GHz` : ''}</span>
							</li>
							{#if h.state === 'unknown'}
								<li>
									<label class="flex items-start gap-2 pl-1">
										<input
											type="checkbox"
											class="mt-0.5"
											checked={unknownOk.has(h.phone_id)}
											disabled={!!ssidDone}
											onchange={() => toggleUnknown(h.phone_id)}
										/>
										<span>{t('unifiWrite.bandConfirm', { name: h.name })}</span>
									</label>
								</li>
							{/if}
						{/each}
					</ul>
					{#if bands.blocked}
						<p class="error-box">{t('unifiWrite.err.handset_band')}</p>
					{/if}
				</div>
			{/if}

			{#if handset}
				<div class="space-y-2">
					<p class="font-medium">{t('unifiWrite.handsetTitle', { name: handset.phone_name })}</p>
					<div class="scroll-x">
						<table class="table">
							<tbody>
								<tr>
									<td class="w-8">
										<input
											type="checkbox"
											aria-label={t('unifiWrite.select', {
												field: t('unifiWrite.field.wifi_roaming_threshold')
											})}
											bind:checked={handsetSelected}
											disabled={!handset.write.allowed || !!handsetDone}
										/>
									</td>
									<td>
										<div>{t('unifiWrite.field.wifi_roaming_threshold')}</div>
										<div class="text-xs text-slate-500 dark:text-slate-400">
											{t(`unifiWrite.rec.${handset.recommendation}` as MessageKey)}
										</div>
									</td>
									<td class="text-slate-500 dark:text-slate-400">
										{valueText(HANDSET_FIELD, handset.effective, labels)}
									</td>
									<td class="font-medium">{valueText(HANDSET_FIELD, handset.proposed, labels)}</td>
								</tr>
							</tbody>
						</table>
					</div>
					{#if handset.write.reason}
						<p class="error-box">{t(`unifiWrite.err.${handset.write.reason}` as MessageKey)}</p>
					{:else}
						<p class="text-xs text-slate-500 dark:text-slate-400">{t('unifiWrite.handsetHint')}</p>
					{/if}
				</div>
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

			{#if proposal.changes.length || handset}
				{#each warnings(proposal, selected) as w (w)}
					<p
						class="rounded-lg bg-amber-50 p-2 text-amber-800 dark:bg-amber-950/40 dark:text-amber-300"
					>
						{t(`unifiWrite.warn.${w}` as MessageKey)}
					</p>
				{/each}
				{#if anything}
					<label class="flex items-start gap-2">
						<input type="checkbox" class="mt-0.5" bind:checked={confirmed} />
						<span>{selected.size ? t('unifiWrite.confirm') : t('unifiWrite.confirmHandset')}</span>
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
