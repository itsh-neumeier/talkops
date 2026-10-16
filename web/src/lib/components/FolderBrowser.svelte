<script lang="ts">
	import { onMount } from 'svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		isArchive,
		joinPath,
		parentPath,
		type BrowseResult,
		type Entry,
		type TargetKind
	} from '#lib/backupTargets.ts';

	let {
		kind,
		start,
		roots = [],
		browse,
		mkdir,
		onSelect,
		onArchive,
		onPinned
	}: {
		kind: TargetKind;
		/** Folder shown first. */
		start: string;
		roots?: string[];
		browse: (path: string) => Promise<BrowseResult>;
		mkdir?: (parent: string, name: string) => Promise<void>;
		/** "Use this folder". */
		onSelect?: (path: string) => void;
		/** Click on a TalkOps archive (restore assistant). */
		onArchive?: (folder: string, name: string) => void;
		/** SFTP: host key accepted on the first connection. */
		onPinned?: (keys: string, fingerprint: string) => void;
	} = $props();

	let current = $state('');
	let entries = $state<Entry[]>([]);
	let truncated = $state(false);
	let loading = $state(false);
	let error = $state('');
	let folder = $state('');

	async function open(path: string) {
		loading = true;
		error = '';
		try {
			const result = await browse(path);
			if (result.host_keys && result.fingerprint) onPinned?.(result.host_keys, result.fingerprint);
			current = path;
			entries = result.entries;
			truncated = result.truncated;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			loading = false;
		}
	}

	async function createFolder() {
		if (!mkdir || !folder.trim() || loading) return;
		error = '';
		try {
			await mkdir(current, folder.trim());
			const name = folder.trim();
			folder = '';
			await open(joinPath(current, name));
		} catch (err) {
			error = errorMessage(err);
		}
	}

	onMount(() => open(start));
	const dirs = $derived(entries.filter((e) => e.is_dir));
	const files = $derived(entries.filter((e) => !e.is_dir));
	const atTop = $derived(current === '');
</script>

<div class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
	<div class="flex flex-wrap items-center gap-2">
		<button
			type="button"
			class="btn btn-sm"
			disabled={atTop || loading}
			onclick={() => open(parentPath(current, kind, roots))}>{t('targets.up')}</button
		>
		<span class="min-w-0 flex-1 truncate font-mono text-sm" title={current}
			>{current || t('targets.top')}</span
		>
		{#if onSelect}
			<button
				type="button"
				class="btn btn-sm btn-primary"
				disabled={loading || (kind === 'local' && atTop)}
				onclick={() => onSelect(current)}>{t('targets.useFolder')}</button
			>
		{/if}
	</div>
	<ErrorBox {error} />
	{#if loading}
		<p class="hint">{t('common.loading')}</p>
	{:else}
		<ul class="max-h-64 divide-y divide-slate-100 overflow-y-auto dark:divide-slate-800">
			{#each dirs as d (d.name)}
				<li>
					<button
						type="button"
						class="w-full truncate px-1 py-1.5 text-left text-sm hover:bg-slate-50 dark:hover:bg-slate-800"
						onclick={() => open(joinPath(current, d.name))}>📁 {d.name}</button
					>
				</li>
			{/each}
			{#each files as f (f.name)}
				<li class="flex items-center justify-between gap-2 px-1 py-1.5 text-sm">
					{#if onArchive && isArchive(f.name)}
						<button
							type="button"
							class="truncate text-left font-mono text-teal-700 hover:underline dark:text-teal-400"
							onclick={() => onArchive(current, f.name)}>{f.name}</button
						>
					{:else}
						<span class="truncate text-slate-500">{f.name}</span>
					{/if}
					<span class="shrink-0 text-xs text-slate-500">{formatDateTime(f.modified)}</span>
				</li>
			{/each}
			{#if !entries.length}<li class="hint py-2">{t('targets.emptyFolder')}</li>{/if}
		</ul>
		{#if truncated}<p class="hint">{t('targets.truncated')}</p>{/if}
	{/if}
	{#if mkdir && !loading && !(kind === 'local' && atTop)}
		<div class="flex gap-2">
			<input
				class="input"
				bind:value={folder}
				placeholder={t('targets.newFolder')}
				aria-label={t('targets.newFolder')}
			/>
			<button type="button" class="btn" disabled={!folder.trim()} onclick={createFolder}
				>{t('targets.createFolder')}</button
			>
		</div>
	{/if}
</div>
