<script lang="ts">
	import { api, type TranscribeOption, type TranscribeOptions } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { choiceKey, formatEta, formatUsd } from '#lib/transcriptions/format.ts';
	import { errorMessage } from '#lib/util.ts';

	/** Model selection with time and cost estimates for one recording. */
	let {
		recording,
		open = $bindable(false),
		onstarted
	}: { recording: string; open?: boolean; onstarted?: () => void } = $props();

	let data = $state<TranscribeOptions | null>(null);
	let selected = $state('');
	let error = $state('');
	let busy = $state(false);
	let started = $state(false);

	const local = $derived(data?.options.filter((o) => o.choice.kind === 'local') ?? []);
	const cloud = $derived(data?.options.filter((o) => o.choice.kind === 'cloud') ?? []);
	const chosen = $derived(data?.options.find((o) => choiceKey(o.choice) === selected));

	$effect(() => {
		if (!open) return;
		data = null;
		error = '';
		started = false;
		api
			.get<TranscribeOptions>(`/recordings/${recording}/transcribe`)
			.then((d) => {
				data = d;
				const first = d.options.find((o) => o.default) ?? d.options[0];
				selected = first ? choiceKey(first.choice) : '';
			})
			.catch((err) => (error = errorMessage(err)));
	});

	function label(o: TranscribeOption): string {
		return o.choice.kind === 'local'
			? t(`stt.local.${o.choice.model}` as MessageKey)
			: `${o.provider} · ${o.choice.model}`;
	}

	async function start(e: SubmitEvent) {
		e.preventDefault();
		if (!chosen) return;
		busy = true;
		error = '';
		try {
			await api.post(`/recordings/${recording}/transcribe`, { engine: chosen.choice });
			started = true;
			onstarted?.();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet option(o: TranscribeOption)}
	{@const key = choiceKey(o.choice)}
	<label
		class="flex cursor-pointer items-start gap-3 rounded-lg border p-3 transition-colors duration-150 motion-reduce:transition-none {selected ===
		key
			? 'border-teal-500 bg-teal-50/70 dark:border-teal-600 dark:bg-teal-950/40'
			: 'border-slate-200 hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-600 dark:hover:bg-slate-800/50'}"
	>
		<input
			type="radio"
			name="engine"
			class="mt-1 h-4 w-4 accent-teal-600"
			value={key}
			bind:group={selected}
		/>
		<span class="min-w-0 flex-1">
			<span class="block text-sm font-medium text-slate-900 dark:text-slate-100">
				{label(o)}
				{#if o.default}<span class="badge badge-muted ml-1 align-middle">{t('tx.default')}</span
					>{/if}
			</span>
			<span class="mt-0.5 flex flex-wrap gap-x-3 text-xs text-slate-500 dark:text-slate-400">
				<span title={o.measured ? t('tx.measured') : t('tx.estimated')}
					>⏱ ≈ {formatEta(o.eta_secs)}
					<span class="text-slate-400 dark:text-slate-500"
						>({o.measured ? t('tx.measured') : t('tx.estimated')})</span
					></span
				>
				{#if o.cost_usd != null}<span>💲 {t('tx.cost', { cost: formatUsd(o.cost_usd) })}</span>{/if}
			</span>
		</span>
	</label>
{/snippet}

<Modal title={t('tx.againTitle')} bind:open>
	{#if started}
		<div class="space-y-4 text-center">
			<div
				class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-emerald-100 text-xl text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300"
				aria-hidden="true"
			>
				✓
			</div>
			<p class="text-sm" role="status">{t('tx.started')}</p>
			<div class="flex justify-center gap-2">
				<button class="btn" onclick={() => (open = false)}>{t('common.close')}</button>
				{#if can('transcriptions.view') || can('transcriptions.manage')}
					<a class="btn btn-primary" href="/transcriptions">{t('tx.showJobs')}</a>
				{/if}
			</div>
		</div>
	{:else}
		<form class="space-y-4" onsubmit={start}>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('tx.againIntro')}</p>
			<ErrorBox {error} />
			{#if !data}
				{#if !error}<Skeleton lines={5} />{/if}
			{:else}
				{#if data.current_engine}
					<p class="text-xs text-slate-500 dark:text-slate-400">
						{t('tx.current', { engine: data.current_engine })}
					</p>
				{/if}
				<fieldset class="space-y-2">
					<legend class="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase"
						>{t('tx.local')}</legend
					>
					{#each local as o (choiceKey(o.choice))}{@render option(o)}{/each}
				</fieldset>
				{#if cloud.length}
					<fieldset class="space-y-2">
						<legend class="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase"
							>{t('tx.cloud')}</legend
						>
						{#each cloud as o (choiceKey(o.choice))}{@render option(o)}{/each}
					</fieldset>
				{/if}
				{#if data.queue_secs > 1}
					<p class="hint">{t('tx.queueWait', { time: formatEta(data.queue_secs) })}</p>
				{/if}
			{/if}
			<div class="flex justify-end gap-2">
				<button type="button" class="btn" onclick={() => (open = false)}
					>{t('common.cancel')}</button
				>
				<button class="btn btn-primary" disabled={!chosen || busy}>{t('tx.start')}</button>
			</div>
		</form>
	{/if}
</Modal>
