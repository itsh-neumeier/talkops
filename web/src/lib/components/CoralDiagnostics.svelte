<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type TranscriptionProvider } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let providers = $state<Pick<TranscriptionProvider, 'id' | 'name'>[]>([]);
	let provider = $state('');
	let minutes = $state(5);
	let includeText = $state(false);
	let busy = $state(false);
	let error = $state('');
	let capture = $state<{ running: boolean; until_unix_ms?: number; events_kept?: number } | null>(
		null
	);

	async function refresh() {
		if (!provider) return;
		error = '';
		try {
			capture = (
				await api.get<{ capture: NonNullable<typeof capture> }>(`/diagnostics/coral/${provider}`)
			).capture;
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(async () => {
		try {
			providers = await api.get<Pick<TranscriptionProvider, 'id' | 'name'>[]>('/diagnostics/coral');
			provider = providers[0]?.id ?? '';
			await refresh();
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function start() {
		busy = true;
		error = '';
		try {
			await api.post(`/diagnostics/coral/${provider}/capture`, {
				minutes: Number(minutes),
				include_text: includeText
			});
			await refresh();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
	async function stop() {
		busy = true;
		error = '';
		try {
			await api.del(`/diagnostics/coral/${provider}/capture`);
			await refresh();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
	async function download() {
		busy = true;
		error = '';
		try {
			const report = await api.get(
				`/diagnostics/coral/${provider}/download?include_text=${includeText}`
			);
			const url = URL.createObjectURL(
				new Blob([JSON.stringify(report, null, 2)], { type: 'application/json' })
			);
			const link = document.createElement('a');
			link.href = url;
			link.download = 'talkops-coral-diagnostics.json';
			link.click();
			setTimeout(() => URL.revokeObjectURL(url), 1000);
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<section class="card space-y-3" data-testid="coral-diagnostics">
	<h2>{t('stt.debugTitle')}</h2>
	<p class="hint">{t('stt.debugHint')}</p>
	<ErrorBox {error} />
	{#if providers.length}
		<div class="flex flex-wrap items-end gap-3">
			<div>
				<label for="coral-debug-provider">{t('stt.provider')}</label>
				<select
					id="coral-debug-provider"
					class="input"
					bind:value={provider}
					onchange={() => {
						capture = null;
						void refresh();
					}}
					disabled={busy}
				>
					{#each providers as item (item.id)}<option value={item.id}>{item.name}</option>{/each}
				</select>
			</div>
			<div>
				<label for="coral-debug-minutes">{t('diag.minutes')}</label>
				<input
					id="coral-debug-minutes"
					class="input w-24"
					type="number"
					min="1"
					max="30"
					bind:value={minutes}
					disabled={busy}
				/>
			</div>
			<button class="btn btn-primary" onclick={start} disabled={busy}>{t('stt.debugStart')}</button>
			<button class="btn" onclick={stop} disabled={busy}>{t('diag.stop')}</button>
			<button class="btn" onclick={refresh} disabled={busy}>{t('stt.debugRefresh')}</button>
			<button class="btn" onclick={download} disabled={busy} data-testid="coral-debug-download"
				>{t('stt.debugDownload')}</button
			>
		</div>
		<label class="flex items-start gap-2 text-sm"
			><input type="checkbox" bind:checked={includeText} disabled={busy} /><span
				>{t('stt.debugIncludeText')}<br /><span class="hint">{t('stt.debugTextHint')}</span></span
			></label
		>
		{#if capture}
			<p class="hint" role="status">
				{capture.running ? t('stt.debugRunning') : t('stt.debugStopped')}
				{#if capture.until_unix_ms}
					· {formatDateTime(new Date(capture.until_unix_ms).toISOString())}{/if}
				· {t('stt.debugEvents', { count: capture.events_kept ?? 0 })}
			</p>
		{/if}
	{:else}
		<p class="hint">{t('stt.debugNoConnection')}</p>
	{/if}
</section>
