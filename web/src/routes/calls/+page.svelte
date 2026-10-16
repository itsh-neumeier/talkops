<script lang="ts">
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import ColumnFilter from '#lib/components/ColumnFilter.svelte';
	import FilterOptions from '#lib/components/FilterOptions.svelte';
	import SortButton from '#lib/components/SortButton.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { syncAddress } from '#lib/list/address.ts';
	import { ariaSort as ariaSortOf, nextSort } from '#lib/list/sort.ts';
	import { api, type Call, type CallParty, type Recording } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import TranscriptView from '#lib/components/TranscriptView.svelte';
	import RetranscribeDialog from '#lib/components/RetranscribeDialog.svelte';
	import { can } from '#lib/session.svelte.ts';
	import { formatDateTime, formatDuration, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		DIRECTIONS,
		RECORDING_FILTERS,
		RESULTS,
		activeCount,
		apiQuery,
		callResult,
		daysAgo,
		emptyFilters,
		pageQuery,
		parseFilters,
		type CallFilters,
		type Direction,
		type RecordingFilter,
		type Result,
		type SortKey
	} from '#lib/calls/filters.ts';

	const PAGE = 50;
	const PHONEBOOK_KEY = 'talkops.calls.phonebook';

	let calls = $state<Call[] | null>(null);
	let total = $state(0);
	let loadingMore = $state(false);
	let error = $state('');
	let open = $state<Recording | null>(null);
	let again = $state(false);
	let filters = $state<CallFilters>(parseFilters(page.url.searchParams));
	let phonebook = $state(true);
	/** Number of the latest list request; older answers are ignored. */
	let generation = 0;

	const active = $derived(activeCount(filters));
	const anyFilter = $derived(Object.values(active).some(Boolean) || !!filters.search.trim());

	onMount(() => {
		try {
			phonebook = localStorage.getItem(PHONEBOOK_KEY) !== 'off';
		} catch {
			// Storage blocked: keep the default.
		}
		load();
	});

	function savePhonebook() {
		try {
			localStorage.setItem(PHONEBOOK_KEY, phonebook ? 'on' : 'off');
		} catch {
			// Storage blocked: the choice applies to this page only.
		}
	}

	async function load() {
		const run = ++generation;
		syncAddress('/calls', pageQuery(filters));
		try {
			const res = await api.getPage<Call>(`/calls${apiQuery(filters, 0, PAGE)}`);
			if (run !== generation) return;
			calls = res.items;
			total = res.total;
			error = '';
		} catch (err) {
			if (run === generation) error = errorMessage(err);
		}
	}

	async function more() {
		if (!calls) return;
		const run = generation;
		loadingMore = true;
		try {
			const res = await api.getPage<Call>(`/calls${apiQuery(filters, calls.length, PAGE)}`);
			if (run !== generation) return;
			const seen = new Set(calls.map((c) => c.id));
			calls = [...calls, ...res.items.filter((c) => !seen.has(c.id))];
			total = res.total;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			loadingMore = false;
		}
	}

	/** Text filters wait for a short typing pause. */
	let typing: ReturnType<typeof setTimeout> | undefined;
	function loadSoon() {
		clearTimeout(typing);
		typing = setTimeout(load, 300);
	}

	function sortBy(key: SortKey) {
		({ sort: filters.sort, order: filters.order } = nextSort(filters, key));
		load();
	}

	function resetAll() {
		const { sort, order } = filters;
		filters = { ...emptyFilters(), sort, order };
		load();
	}

	function quickRange(days: number) {
		filters.since = daysAgo(days);
		filters.until = '';
		load();
	}

	async function toggleRecording(c: Call) {
		if (!c.recording_id || open?.id === c.recording_id) {
			open = null;
			return;
		}
		try {
			open = await api.get<Recording>(`/recordings/${c.recording_id}`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(r: Recording) {
		if (!confirm(t('rec.deleteConfirm'))) return;
		try {
			await api.del(`/recordings/${r.id}`);
			open = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	const arrow = { inbound: '↙', outbound: '↗', internal: '↔' } as const;
	const contactHref = (id: string) => `/phonebook?contact=${encodeURIComponent(id)}`;
	const ariaSort = (key: SortKey) => ariaSortOf(filters.sort === key, filters.order);
</script>

<!-- One side of a call: phone book name over the number, or number over the name. -->
{#snippet party(p: CallParty | undefined, rawNumber: string, rawName: string)}
	{#if p && p.extension_id}
		<span class="font-mono">{p.number}</span>
		{#if p.name}<div class="text-xs text-slate-500 dark:text-slate-400">{p.name}</div>{/if}
	{:else if phonebook && p && p.name && p.contact_id}
		<a
			class="font-medium decoration-slate-300 decoration-dotted underline-offset-2 hover:text-teal-700 hover:underline dark:decoration-slate-600 dark:hover:text-teal-300"
			href={contactHref(p.contact_id)}
			title={t('dash.party.openEntry', { name: p.name })}>{p.name}</a
		>
		<div class="font-mono text-xs text-slate-500 dark:text-slate-400">{p.number || rawNumber}</div>
	{:else}
		<span class="font-mono">{rawNumber || '—'}</span>
		{#if rawName && rawName !== rawNumber}
			<div class="text-xs text-slate-500 dark:text-slate-400">{rawName}</div>
		{/if}
	{/if}
{/snippet}

<div class="space-y-4">
	<PageHeader title={t('nav.calls')}>
		{#snippet actions()}
			<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
				<label class="inline-flex cursor-pointer items-center gap-2 select-none">
					<input
						type="checkbox"
						class="h-4 w-4 accent-teal-600"
						bind:checked={phonebook}
						onchange={savePhonebook}
					/>
					{t('calls.phonebookNames')}
				</label>
				<form
					class="flex min-w-0 flex-[1_1_14rem] gap-2"
					onsubmit={(e) => {
						e.preventDefault();
						load();
					}}
				>
					<input
						class="input mt-0 min-w-0 flex-1"
						type="search"
						bind:value={filters.search}
						placeholder={t('common.search')}
						aria-label={t('common.search')}
					/>
					<button class="btn">{t('common.search')}</button>
				</form>
			</div>
		{/snippet}
	</PageHeader>
	<ErrorBox {error} />
	<div class="card scroll-x">
		<table class="table">
			<thead>
				<tr>
					<th aria-sort={ariaSort('time')}>
						<span class="inline-flex items-center gap-1 whitespace-nowrap">
							<SortButton
								label={t('calls.time')}
								active={filters.sort === 'time'}
								order={filters.order}
								onclick={() => sortBy('time')}
							/>
							<ColumnFilter
								label={t('calls.time')}
								active={active.time}
								onreset={() => {
									filters.since = '';
									filters.until = '';
									load();
								}}
							>
								<div class="mb-3 flex flex-wrap gap-1">
									<button type="button" class="btn btn-sm" onclick={() => quickRange(0)}
										>{t('calls.range.today')}</button
									>
									<button type="button" class="btn btn-sm" onclick={() => quickRange(6)}
										>{t('calls.range.week')}</button
									>
									<button type="button" class="btn btn-sm" onclick={() => quickRange(29)}
										>{t('calls.range.month')}</button
									>
								</div>
								<div class="grid grid-cols-2 gap-2">
									<label
										>{t('calls.range.since')}
										<input class="input" type="date" bind:value={filters.since} onchange={load} />
									</label>
									<label
										>{t('calls.range.until')}
										<input class="input" type="date" bind:value={filters.until} onchange={load} />
									</label>
								</div>
							</ColumnFilter>
						</span>
					</th>
					<th>
						<span class="inline-flex items-center gap-1 whitespace-nowrap">
							{t('calls.direction')}
							<ColumnFilter
								label={t('calls.direction')}
								active={active.direction}
								onreset={() => {
									filters.direction = [];
									load();
								}}
							>
								<FilterOptions
									options={DIRECTIONS.map((d) => ({
										value: d,
										label: t(`dir.${d}`),
										icon: arrow[d]
									}))}
									selected={filters.direction}
									onchange={(v) => {
										filters.direction = v as Direction[];
										load();
									}}
								/>
							</ColumnFilter>
						</span>
					</th>
					<th>
						<span class="inline-flex items-center gap-1 whitespace-nowrap">
							{t('calls.from')}
							<ColumnFilter
								label={t('calls.from')}
								active={active.from}
								onreset={() => {
									filters.from = '';
									load();
								}}
							>
								<label
									>{t('calls.filterText')}
									<input
										class="input"
										type="search"
										bind:value={filters.from}
										oninput={loadSoon}
										placeholder={t('calls.filterTextPlaceholder')}
									/>
								</label>
							</ColumnFilter>
						</span>
					</th>
					<th>
						<span class="inline-flex items-center gap-1 whitespace-nowrap">
							{t('calls.to')}
							<ColumnFilter
								label={t('calls.to')}
								active={active.to}
								onreset={() => {
									filters.to = '';
									load();
								}}
							>
								<label
									>{t('calls.filterText')}
									<input
										class="input"
										type="search"
										bind:value={filters.to}
										oninput={loadSoon}
										placeholder={t('calls.filterTextPlaceholder')}
									/>
								</label>
								<p class="hint">{t('calls.filterToHint')}</p>
							</ColumnFilter>
						</span>
					</th>
					<th class="col-opt" aria-sort={ariaSort('duration')}>
						<SortButton
							label={t('calls.duration')}
							active={filters.sort === 'duration'}
							order={filters.order}
							onclick={() => sortBy('duration')}
						/>
					</th>
					<th>
						<span class="inline-flex items-center gap-1 whitespace-nowrap">
							{t('calls.result')}
							<ColumnFilter
								label={t('calls.result')}
								active={active.result}
								onreset={() => {
									filters.result = [];
									load();
								}}
							>
								<FilterOptions
									options={RESULTS.map((r) => ({ value: r, label: t(`calls.result.${r}`) }))}
									selected={filters.result}
									onchange={(v) => {
										filters.result = v as Result[];
										load();
									}}
								/>
							</ColumnFilter>
						</span>
					</th>
					<th class="text-right">
						<span class="inline-flex items-center gap-1 whitespace-nowrap">
							<span class="sr-only">{t('rec.recording')}</span>
							<ColumnFilter
								label={t('rec.recording')}
								active={active.recording}
								onreset={() => {
									filters.recording = '';
									load();
								}}
							>
								<FilterOptions
									single
									options={(['', ...RECORDING_FILTERS] as const).map((r) => ({
										value: r,
										label: t(r ? `calls.recording.${r}` : 'common.all')
									}))}
									selected={[filters.recording]}
									onchange={([v]) => {
										filters.recording = v as RecordingFilter;
										load();
									}}
								/>
							</ColumnFilter>
						</span>
					</th>
				</tr>
			</thead>
			<tbody>
				{#each calls ?? [] as c (c.id)}
					{@const result = callResult(c)}
					<tr class="row-in">
						<td class="whitespace-nowrap">{formatDateTime(c.started_at)}</td>
						<td title={t(`dir.${c.direction}`)}
							>{arrow[c.direction]}
							<span class="hidden @xl:inline">{t(`dir.${c.direction}`)}</span></td
						>
						<td>{@render party(c.caller, c.caller_number, c.caller_name)}</td>
						<td>
							{#if c.direction === 'inbound' && c.called?.extension_id}
								<div class="flex flex-wrap items-baseline gap-x-1">
									<span class="whitespace-nowrap"
										>{t('calls.extensionShort')}&nbsp;<span class="font-mono"
											>{c.called.number}</span
										></span
									>
									{#if c.via_number}
										<span class="text-slate-500 dark:text-slate-400">{t('calls.via')}</span>
										<span class="font-mono whitespace-nowrap">{c.via_number}</span>
									{/if}
								</div>
								{#if c.called.name}<div class="text-xs text-slate-500 dark:text-slate-400">
										{c.called.name}
									</div>{/if}
							{:else}
								{@render party(c.called, c.destination, '')}
							{/if}
						</td>
						<td class="col-opt font-mono">{c.answered_at ? formatDuration(c.billsec) : '—'}</td>
						<td>
							{#if result === 'answered'}<span class="badge whitespace-nowrap badge-ok">OK</span>
							{:else if result === 'busy'}<span
									class="badge whitespace-nowrap badge-warn"
									title={c.hangup_cause}>{t('calls.result.busy')}</span
								>
							{:else}<span class="badge whitespace-nowrap badge-bad" title={c.hangup_cause}
									>{t('calls.missed')}</span
								>{/if}
						</td>
						<td class="text-right">
							{#if c.recording_id}
								<button
									class="btn btn-sm whitespace-nowrap"
									aria-expanded={open?.id === c.recording_id}
									aria-label={t('rec.recording')}
									onclick={() => toggleRecording(c)}
									>▶<span class="hidden @3xl:inline"> {t('rec.recording')}</span></button
								>
							{/if}
						</td>
					</tr>
					{#if open && open.id === c.recording_id}
						<tr>
							<td colspan="7" class="expand-in space-y-3 bg-slate-50 dark:bg-slate-900/40">
								<audio
									class="w-full"
									controls
									preload="none"
									src="/api/v1/recordings/{open.id}/audio"
								></audio>
								<TranscriptView
									url="/recordings/{open.id}/transcript"
									status={open.transcript_status}
								/>
								<div class="flex flex-wrap justify-end gap-1">
									{#if can('recordings.retranscribe')}
										<button class="btn btn-sm" onclick={() => (again = true)}
											>{t('tx.again')}</button
										>
									{/if}
									<a class="btn btn-sm" href="/api/v1/recordings/{open.id}/audio" download
										>{t('vm.download')}</a
									>
									{#if can('recordings.delete')}
										<button class="btn btn-sm btn-danger" onclick={() => remove(open!)}
											>{t('common.delete')}</button
										>
									{/if}
								</div>
							</td>
						</tr>
					{/if}
				{:else}
					{#if calls === null || !net.settled}<SkeletonRows cols={7} />
					{:else}
						<tr>
							<td colspan="7" class="py-8 text-center text-sm text-slate-500 dark:text-slate-400">
								{anyFilter ? t('calls.noMatches') : t('calls.empty')}
								{#if anyFilter}
									<div class="mt-2">
										<button class="btn btn-sm" onclick={resetAll}>{t('filter.resetAll')}</button>
									</div>
								{/if}
							</td>
						</tr>
					{/if}
				{/each}
			</tbody>
		</table>
		{#if calls && calls.length > 0}
			<div
				class="flex flex-wrap items-center justify-between gap-2 pt-3 text-sm text-slate-500 dark:text-slate-400"
			>
				<span>{t('calls.shown', { shown: calls.length, total })}</span>
				<div class="flex gap-2">
					{#if anyFilter}
						<button class="btn btn-sm" onclick={resetAll}>{t('filter.resetAll')}</button>
					{/if}
					{#if calls.length < total}
						<button class="btn btn-sm" disabled={loadingMore} onclick={more}
							>{t('calls.more')}</button
						>
					{/if}
				</div>
			</div>
		{/if}
	</div>
</div>

{#if open}
	<RetranscribeDialog recording={open.id} bind:open={again} />
{/if}
