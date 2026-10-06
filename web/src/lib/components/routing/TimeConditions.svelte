<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type HolidayCalendar, type TimeCondition } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { describe, loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const days = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'] as const;
	const officeHours = () =>
		Object.fromEntries(
			days.map((d) => [d, d === 'sat' || d === 'sun' ? [] : [['08:00', '17:00']]])
		) as Record<string, [string, string][]>;
	const blank = () => ({
		id: '',
		number: '',
		name: '',
		schedule: officeHours(),
		holiday_region: 'DE' as string | null,
		closed_dates: '',
		override: 'auto' as TimeCondition['override'],
		open_type: 'none' as TimeCondition['open_type'],
		open_id: null as string | null,
		closed_type: 'none' as TimeCondition['closed_type'],
		closed_id: null as string | null
	});
	let form = $state(blank());
	let open = $state(false);
	let error = $state('');
	let regions = $state<[string, string][]>([]);

	onMount(async () => {
		regions = (await api.get<HolidayCalendar>('/holidays')).regions;
	});

	function edit(c: TimeCondition | null) {
		error = '';
		form = c
			? {
					...c,
					number: c.number ?? '',
					schedule: Object.fromEntries(days.map((d) => [d, [...(c.schedule[d] ?? [])]])),
					closed_dates: c.closed_dates.join('\n')
				}
			: blank();
		open = true;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			...form,
			number: form.number || null,
			holiday_region: form.holiday_region || null,
			closed_dates: form.closed_dates
				.split(/[\s,]+/)
				.map((d) => d.trim())
				.filter(Boolean)
		};
		try {
			if (form.id) await api.put(`/time-conditions/${form.id}`, body);
			else await api.post('/time-conditions', body);
			open = false;
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function override(c: TimeCondition, value: TimeCondition['override']) {
		try {
			await api.put(`/time-conditions/${c.id}/override`, { override: value });
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(c: TimeCondition) {
		if (!confirm(t('common.confirmDelete', { name: c.name }))) return;
		try {
			await api.del(`/time-conditions/${c.id}`);
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function stateLabel(c: TimeCondition) {
		const s = c.state;
		const base = s.open ? t('tc.open') : t('tc.closed');
		if (s.reason === 'holiday') return `${base} (${s.holiday})`;
		if (s.reason === 'override') return `${base} (${t('tc.forced')})`;
		if (s.reason === 'closed_date') return `${base} (${t('tc.closedDate')})`;
		return base;
	}
</script>

<div class="space-y-3">
	<div class="flex items-center justify-between gap-2">
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('tc.hint')}</p>
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={() => edit(null)}
				>{t('tc.new')}</button
			>{/if}
	</div>
	<ErrorBox error={open ? '' : error} />
	<div class="card overflow-x-auto">
		{#if targets.conditions.length === 0}
			<p class="text-sm text-slate-500">{t('routing.none')}</p>
		{:else}
			<table class="table">
				<thead
					><tr
						><th>{t('routing.number')}</th><th>{t('common.name')}</th><th>{t('tc.state')}</th><th
							>{t('tc.whenOpen')}</th
						><th>{t('tc.whenClosed')}</th><th></th></tr
					></thead
				>
				<tbody>
					{#each targets.conditions as c (c.id)}
						<tr>
							<td class="font-mono">{c.number ?? '—'}</td>
							<td>{c.name}</td>
							<td
								><span class="badge {c.state.open ? 'badge-ok' : 'badge-muted'}"
									>{stateLabel(c)}</span
								></td
							>
							<td>{describe(c.open_type, c.open_id)?.label ?? '—'}</td>
							<td>{describe(c.closed_type, c.closed_id)?.label ?? '—'}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								<select
									class="input mt-0 inline-block w-auto py-1"
									value={c.override}
									onchange={(e) => override(c, e.currentTarget.value as TimeCondition['override'])}
									aria-label={t('tc.override')}
								>
									<option value="auto">{t('tc.auto')}</option>
									<option value="open">{t('tc.forceOpen')}</option>
									<option value="closed">{t('tc.forceClosed')}</option>
								</select>
								{#if hasRole('admin')}
									<button class="btn btn-sm" onclick={() => edit(c)}>{t('common.edit')}</button>
									<button class="btn btn-sm btn-danger" onclick={() => remove(c)}
										>{t('common.delete')}</button
									>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>

<Modal title={form.id ? t('common.edit') : t('tc.new')} bind:open>
	<form class="space-y-3" onsubmit={save}>
		<ErrorBox {error} />
		<div class="grid grid-cols-3 gap-3">
			<div>
				<label for="tc-num">{t('routing.number')}</label><input
					id="tc-num"
					class="input font-mono"
					bind:value={form.number}
					placeholder="60"
				/>
			</div>
			<div class="col-span-2">
				<label for="tc-name">{t('common.name')}</label><input
					id="tc-name"
					class="input"
					bind:value={form.name}
					required
				/>
			</div>
		</div>
		<p class="hint">{t('tc.numberHint')}</p>
		<fieldset class="space-y-1">
			<legend class="text-sm font-medium">{t('tc.hours')}</legend>
			{#each days as d (d)}
				<div class="flex flex-wrap items-center gap-2 text-sm">
					<span class="w-10">{t(`day.${d}` as MessageKey)}</span>
					{#each form.schedule[d] as range, i (i)}
						<span class="flex items-center gap-1">
							<input
								class="input mt-0 w-32 py-1"
								type="time"
								bind:value={range[0]}
								aria-label={t('tc.from')}
							/>–<input
								class="input mt-0 w-32 py-1"
								type="time"
								bind:value={range[1]}
								aria-label={t('tc.to')}
							/>
							<button
								type="button"
								class="btn btn-sm"
								onclick={() => (form.schedule[d] = form.schedule[d].filter((_, j) => j !== i))}
								aria-label={t('common.delete')}>✕</button
							>
						</span>
					{/each}
					<button
						type="button"
						class="btn btn-sm"
						onclick={() => (form.schedule[d] = [...form.schedule[d], ['08:00', '17:00']])}>+</button
					>
					{#if form.schedule[d].length === 0}<span class="text-slate-500">{t('tc.closed')}</span
						>{/if}
				</div>
			{/each}
		</fieldset>
		<div>
			<label for="tc-region">{t('tc.holidays')}</label>
			<select id="tc-region" class="input" bind:value={form.holiday_region}>
				<option value={null}>{t('tc.noHolidays')}</option>
				{#each regions as [code, name] (code)}<option value={code}>{name}</option>{/each}
			</select>
		</div>
		<div>
			<label for="tc-dates">{t('tc.closedDates')}</label>
			<textarea
				id="tc-dates"
				class="input font-mono"
				rows="3"
				bind:value={form.closed_dates}
				placeholder="2026-12-24&#10;2026-12-31"></textarea>
			<p class="hint">{t('tc.closedDatesHint')}</p>
		</div>
		<div class="grid gap-3 sm:grid-cols-2">
			<div>
				<label for="tc-open">{t('tc.whenOpen')}</label><DestinationSelect
					inputId="tc-open"
					bind:type={form.open_type}
					bind:id={form.open_id}
					exclude={form.id}
					noneLabel={t('routing.hangup')}
				/>
			</div>
			<div>
				<label for="tc-closed">{t('tc.whenClosed')}</label><DestinationSelect
					inputId="tc-closed"
					bind:type={form.closed_type}
					bind:id={form.closed_id}
					exclude={form.id}
					noneLabel={t('routing.hangup')}
				/>
			</div>
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>
