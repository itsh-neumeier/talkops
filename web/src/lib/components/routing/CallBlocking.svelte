<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Block = { id: string; pattern: string; label: string; created_at: string };
	type Settings = {
		block_anonymous: boolean;
		phoneblock_enabled: boolean;
		phoneblock_token_set: boolean;
		phoneblock_min_votes: number;
	};

	let blocks = $state<Block[]>([]);
	let settings = $state<Settings | null>(null);
	let token = $state('');
	let pattern = $state('');
	let label = $state('');
	let testNumber = $state('');
	let testResult = $state('');
	let error = $state('');
	let saved = $state(false);

	async function load() {
		try {
			blocks = await api.get<Block[]>('/call-blocks');
			settings = await api.get<Settings>('/call-blocks/settings');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function add(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			await api.post('/call-blocks', { pattern, label });
			pattern = '';
			label = '';
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(b: Block) {
		if (!confirm(t('common.confirmDelete', { name: b.pattern }))) return;
		try {
			await api.del(`/call-blocks/${b.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function saveSettings(e: SubmitEvent) {
		e.preventDefault();
		if (!settings) return;
		error = '';
		saved = false;
		try {
			settings = await api.put<Settings>('/call-blocks/settings', {
				block_anonymous: settings.block_anonymous,
				phoneblock_enabled: settings.phoneblock_enabled,
				phoneblock_token: token ? token : undefined,
				phoneblock_min_votes: Number(settings.phoneblock_min_votes)
			});
			token = '';
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeToken() {
		if (!settings) return;
		try {
			settings = await api.put<Settings>('/call-blocks/settings', {
				block_anonymous: settings.block_anonymous,
				phoneblock_enabled: false,
				phoneblock_token: '',
				phoneblock_min_votes: Number(settings.phoneblock_min_votes)
			});
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function test(e: SubmitEvent) {
		e.preventDefault();
		testResult = '';
		try {
			const r = await api.post<{ blocked: boolean; reason: string | null }>('/call-blocks/test', {
				number: testNumber
			});
			testResult = r.blocked
				? t('block.testBlocked', { reason: r.reason ?? '' })
				: t('block.testAllowed');
		} catch (err) {
			testResult = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<ErrorBox {error} />

	<section class="card space-y-3">
		<h2>{t('block.list')}</h2>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('block.listHint')}</p>
		<form class="flex flex-wrap items-end gap-2" onsubmit={add}>
			<div>
				<label for="b-pattern">{t('block.pattern')}</label>
				<input
					id="b-pattern"
					class="input font-mono"
					placeholder="+4930123456 / +49900*"
					bind:value={pattern}
					required
				/>
			</div>
			<div class="min-w-48 flex-1">
				<label for="b-label">{t('block.label')}</label>
				<input id="b-label" class="input" maxlength="100" bind:value={label} />
			</div>
			<button class="btn btn-primary">{t('block.add')}</button>
		</form>
		{#if blocks.length}
			<table class="w-full text-left text-sm">
				<thead class="text-slate-500 dark:text-slate-400">
					<tr
						><th class="py-1">{t('block.pattern')}</th><th class="py-1">{t('block.label')}</th><th
							class="py-1">{t('block.added')}</th
						><th></th></tr
					>
				</thead>
				<tbody>
					{#each blocks as b (b.id)}
						<tr class="border-t border-slate-200 dark:border-slate-700">
							<td class="py-1.5 font-mono">{b.pattern}</td>
							<td class="py-1.5">{b.label}</td>
							<td class="py-1.5 text-slate-500 dark:text-slate-400"
								>{formatDateTime(b.created_at)}</td
							>
							<td class="py-1.5 text-right"
								><button class="btn btn-danger btn-sm" onclick={() => remove(b)}
									>{t('common.delete')}</button
								></td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('block.empty')}</p>
		{/if}
	</section>

	{#if settings}
		<form class="card space-y-3" onsubmit={saveSettings}>
			<h2>{t('block.sources')}</h2>
			{#if saved}<p class="text-sm text-emerald-700 dark:text-emerald-400">
					{t('common.saved')}
				</p>{/if}
			<label class="flex items-center gap-2"
				><input
					type="checkbox"
					bind:checked={settings.block_anonymous}
					disabled={!hasRole('admin')}
				/>
				{t('block.anonymous')}</label
			>
			<fieldset class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
				<legend class="px-1 text-sm font-medium">PhoneBlock</legend>
				<p class="text-sm text-slate-600 dark:text-slate-300">
					{t('block.phoneblockHint')}
					<a
						class="text-teal-700 underline dark:text-teal-300"
						href="https://phoneblock.net/phoneblock/"
						target="_blank"
						rel="noopener noreferrer">phoneblock.net</a
					>
				</p>
				<label class="flex items-center gap-2"
					><input
						type="checkbox"
						bind:checked={settings.phoneblock_enabled}
						disabled={!hasRole('admin')}
					/>
					{t('block.phoneblockEnable')}</label
				>
				<div class="grid gap-3 sm:grid-cols-2">
					<div>
						<label for="b-token">{t('block.token')}</label>
						<input
							id="b-token"
							class="input font-mono"
							type="password"
							autocomplete="off"
							placeholder={settings.phoneblock_token_set ? t('block.tokenKeep') : ''}
							bind:value={token}
							disabled={!hasRole('admin')}
						/>
						{#if settings.phoneblock_token_set && hasRole('admin')}
							<button
								type="button"
								class="mt-1 text-xs text-red-600 underline"
								onclick={removeToken}>{t('block.tokenRemove')}</button
							>
						{/if}
					</div>
					<div>
						<label for="b-votes">{t('block.minVotes')}</label>
						<input
							id="b-votes"
							class="input w-28"
							type="number"
							min="1"
							max="100"
							bind:value={settings.phoneblock_min_votes}
							disabled={!hasRole('admin')}
						/>
						<p class="hint">{t('block.minVotesHint')}</p>
					</div>
				</div>
			</fieldset>
			{#if hasRole('admin')}
				<div class="flex justify-end">
					<button class="btn btn-primary">{t('common.save')}</button>
				</div>
			{/if}
		</form>
	{/if}

	<form class="card flex flex-wrap items-end gap-2" onsubmit={test}>
		<div>
			<label for="b-test">{t('block.test')}</label>
			<input
				id="b-test"
				class="input font-mono"
				placeholder="+4930123456"
				bind:value={testNumber}
			/>
		</div>
		<button class="btn">{t('block.testRun')}</button>
		{#if testResult}<p class="w-full text-sm" data-testid="block-test">{testResult}</p>{/if}
	</form>
</div>
