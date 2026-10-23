<script lang="ts">
	// History of changes to the UniFi voice SSID (ADR 0042) and to handset
	// settings (ADR 0043) with rollback. A rollback that finds changes made
	// since asks again; one that removes a band shows the band check.
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api.ts';
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import {
		canRollBack,
		slotText,
		writeErrorKey,
		type ChangeField,
		type ForeignField,
		type HandsetBand,
		type WifiChange
	} from '#lib/unifiWrite.ts';
	import { errorMessage } from '#lib/util.ts';

	let changes = $state<WifiChange[] | null>(null);
	let error = $state('');
	let expanded = $state<string | null>(null);

	// Rollback dialog.
	let target = $state<WifiChange | null>(null);
	let rollbackOpen = $state(false);
	let foreign = $state<ForeignField[]>([]);
	let handsets = $state<HandsetBand[]>([]);
	let handsetsOk = $state<string[]>([]);
	let acceptForeign = $state(false);
	let busy = $state(false);
	let rollbackError = $state('');

	const labels = $derived({
		on: t('unifiWrite.value.on'),
		off: t('unifiWrite.value.off'),
		absent: t('unifiWrite.value.absent')
	});
	const fieldName = (f: ChangeField) => t(`unifiWrite.field.${f}` as MessageKey);
	const stateBadge = (s: WifiChange['state']) =>
		s === 'applied'
			? 'badge badge-ok'
			: s === 'failed'
				? 'badge badge-bad'
				: s === 'applying' || s === 'waiting'
					? 'badge badge-warn'
					: 'badge badge-muted';

	function message(err: unknown): string {
		const key = writeErrorKey(err);
		return key ? t(key as MessageKey) : errorMessage(err);
	}

	async function load() {
		try {
			changes = await api.get<WifiChange[]>('/unifi/wifi-changes');
			error = '';
		} catch (err) {
			error = message(err);
		}
	}

	onMount(load);

	function openRollback(c: WifiChange) {
		target = c;
		foreign = [];
		handsets = [];
		handsetsOk = [];
		acceptForeign = false;
		rollbackError = '';
		rollbackOpen = true;
	}

	async function rollback(accept: boolean) {
		if (!target) return;
		busy = true;
		rollbackError = '';
		acceptForeign = accept;
		try {
			await api.post(`/unifi/wifi-changes/${target.id}/rollback`, {
				confirm: true,
				accept_foreign_changes: accept,
				confirm_unknown_handsets: handsetsOk
			});
			rollbackOpen = false;
			await load();
		} catch (err) {
			if (err instanceof ApiError && err.code === 'foreign_change') {
				foreign = (err.body.fields as ForeignField[]) ?? [];
			} else if (
				err instanceof ApiError &&
				(err.code === 'handset_band' || err.code === 'handset_unknown')
			) {
				handsets = (err.body.handsets as HandsetBand[]) ?? [];
				rollbackError = message(err);
			} else {
				rollbackError = message(err);
			}
		} finally {
			busy = false;
		}
	}

	function toggleHandset(id: string) {
		handsetsOk = handsetsOk.includes(id) ? handsetsOk.filter((x) => x !== id) : [...handsetsOk, id];
	}

	const bandBlocked = $derived(handsets.some((h) => h.state === 'blocked'));
	const bandOpen = $derived(
		handsets.some((h) => h.state === 'unknown' && !handsetsOk.includes(h.phone_id))
	);
</script>

<div class="space-y-4">
	<ErrorBox {error} />
	<section class="card space-y-2 p-4">
		<h2>{t('unifiWrite.historyTitle')}</h2>
		<p class="text-sm text-slate-500 dark:text-slate-400">{t('unifiWrite.historyIntro')}</p>
	</section>
	<div class="card scroll-x">
		<table class="table">
			<thead>
				<tr>
					<th>{t('unifiWrite.col.time')}</th>
					<th class="col-opt">{t('unifiWrite.col.user')}</th>
					<th class="col-opt">{t('unifiWrite.col.ssid')}</th>
					<th>{t('unifiWrite.col.fields')}</th>
					<th>{t('unifiWrite.col.state')}</th>
					<th><span class="sr-only">{t('unifiWrite.col.actions')}</span></th>
				</tr>
			</thead>
			<tbody>
				{#if !changes}
					<SkeletonRows cols={6} />
				{:else if !changes.length}
					<EmptyRow cols={6} text={t('unifiWrite.historyEmpty')} />
				{:else}
					{#each changes as c (c.id)}
						<tr>
							<td>
								<button
									class="text-left font-medium hover:underline"
									aria-expanded={expanded === c.id}
									onclick={() => (expanded = expanded === c.id ? null : c.id)}
									>{formatDateTime(c.created_at)}</button
								>
								{#if c.kind === 'rollback'}
									<span class="badge badge-muted">{t('unifiWrite.kind.rollback')}</span>
								{/if}
							</td>
							<td class="col-opt">{c.created_by_name ?? '—'}</td>
							<td class="col-opt">
								{c.target === 'handset'
									? `${t('unifiWrite.targetHandset')}: ${c.phone_name || '—'}`
									: c.wifi_name || '—'}
							</td>
							<td>
								{c.fields.map(fieldName).join(', ')}
								{#if c.target === 'handset'}
									<div class="text-xs text-slate-500 dark:text-slate-400">{c.phone_name}</div>
								{/if}
							</td>
							<td>
								<span class={stateBadge(c.state)}>{t(`unifiWrite.state.${c.state}`)}</span>
								{#if c.error}
									<div class="text-xs text-slate-500 dark:text-slate-400">
										{writeErrorKey({ code: c.error })
											? t(writeErrorKey({ code: c.error }) as MessageKey)
											: c.error}
									</div>
								{/if}
							</td>
							<td class="text-right">
								{#if canRollBack(c) && can('unifi.write')}
									<button class="btn btn-sm" onclick={() => openRollback(c)}
										>{t('unifiWrite.rollback')}</button
									>
								{/if}
							</td>
						</tr>
						{#if expanded === c.id}
							<tr class="expand-in">
								<td colspan="6">
									<dl class="grid grid-cols-1 gap-x-4 gap-y-1 text-sm @lg:grid-cols-3">
										{#each c.fields as f (f)}
											<dt class="font-medium">{fieldName(f)}</dt>
											<dd>{t('unifiWrite.before')}: {slotText(f, c.before?.[f], labels)}</dd>
											<dd>{t('unifiWrite.after')}: {slotText(f, c.after?.[f], labels)}</dd>
										{/each}
									</dl>
									{#if c.survey_id}
										<a class="text-sm hover:underline" href={`/wifi-surveys/${c.survey_id}`}
											>{t('unifiWrite.survey')}</a
										>
									{/if}
								</td>
							</tr>
						{/if}
					{/each}
				{/if}
			</tbody>
		</table>
	</div>
</div>

<Modal title={t('unifiWrite.rollbackTitle')} bind:open={rollbackOpen}>
	{#if target}
		<div class="space-y-3 text-sm">
			<ErrorBox error={rollbackError} />
			{#if foreign.length}
				<h3 class="font-medium">{t('unifiWrite.foreignTitle')}</h3>
				<p>{t('unifiWrite.foreignIntro')}</p>
				<div class="scroll-x">
					<table class="table">
						<thead>
							<tr>
								<th>{t('unifiWrite.col.field')}</th>
								<th>{t('unifiWrite.col.afterTalkops')}</th>
								<th>{t('unifiWrite.col.now')}</th>
								<th>{t('unifiWrite.col.restore')}</th>
							</tr>
						</thead>
						<tbody>
							{#each foreign as f (f.field)}
								<tr>
									<td>{fieldName(f.field)}</td>
									<td>{slotText(f.field, f.after, labels)}</td>
									<td class="font-medium">{slotText(f.field, f.current, labels)}</td>
									<td>{slotText(f.field, f.before, labels)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else}
				<p>{t('unifiWrite.rollbackIntro')}</p>
				<ul class="list-disc space-y-1 pl-5">
					{#each target.fields as f (f)}
						<li>{fieldName(f)}: {slotText(f, target.before?.[f], labels)}</li>
					{/each}
				</ul>
			{/if}
			{#if handsets.length}
				<ul class="space-y-1">
					{#each handsets as h (h.phone_id)}
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
										checked={handsetsOk.includes(h.phone_id)}
										onchange={() => toggleHandset(h.phone_id)}
									/>
									<span>{t('unifiWrite.bandConfirm', { name: h.name })}</span>
								</label>
							</li>
						{/if}
					{/each}
				</ul>
			{/if}
			{#if target.target === 'handset'}
				<p class="text-xs text-slate-500 dark:text-slate-400">{t('unifiWrite.handsetHint')}</p>
			{:else}
				<p
					class="rounded-lg bg-amber-50 p-2 text-amber-800 dark:bg-amber-950/40 dark:text-amber-300"
				>
					{t('unifiWrite.warn.disconnect')}
				</p>
			{/if}
			<div class="flex justify-end gap-2">
				<button class="btn" onclick={() => (rollbackOpen = false)}>{t('common.cancel')}</button>
				{#if foreign.length}
					<button
						class="btn btn-danger"
						disabled={busy || bandBlocked || bandOpen}
						onclick={() => rollback(true)}>{t('unifiWrite.rollbackAnyway')}</button
					>
				{:else}
					<button
						class="btn btn-primary"
						disabled={busy || bandBlocked || bandOpen}
						onclick={() => rollback(acceptForeign)}>{t('unifiWrite.rollbackConfirm')}</button
					>
				{/if}
			</div>
		</div>
	{/if}
</Modal>
