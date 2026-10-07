<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Settings = {
		enabled: boolean;
		max_failures: number;
		window_minutes: number;
		ban_minutes: number;
		trusted_networks: string[];
	};
	type Ban = {
		ip: string;
		failures: number;
		last_user: string;
		banned_at: string;
		banned_until: string;
	};

	let s = $state<Settings | null>(null);
	let bans = $state<Ban[]>([]);
	let trusted = $state('');
	let error = $state('');
	let info = $state('');

	async function load() {
		try {
			const data = await api.get<{ settings: Settings; bans: Ban[] }>('/security/sip');
			s = data.settings;
			bans = data.bans;
			trusted = data.settings.trusted_networks.join('\n');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!s) return;
		error = '';
		info = '';
		try {
			s = await api.put<Settings>('/security/sip', {
				enabled: s.enabled,
				max_failures: Number(s.max_failures),
				window_minutes: Number(s.window_minutes),
				ban_minutes: Number(s.ban_minutes),
				trusted_networks: trusted
					.split(/[\s,]+/)
					.map((n) => n.trim())
					.filter(Boolean)
			});
			trusted = s.trusted_networks.join('\n');
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function unban(ban: Ban) {
		error = '';
		try {
			await api.del(`/security/sip/bans/${encodeURIComponent(ban.ip)}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

{#if s}
	<form class="card space-y-4" onsubmit={save}>
		<h2>{t('guard.title')}</h2>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('guard.hint')}</p>
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
		<label class="flex items-center gap-2 font-medium"
			><input type="checkbox" bind:checked={s.enabled} /> {t('guard.enabled')}</label
		>
		<div class="grid gap-3 sm:grid-cols-3">
			<div>
				<label for="g-max">{t('guard.maxFailures')}</label>
				<input
					id="g-max"
					class="input"
					type="number"
					min="3"
					max="1000"
					bind:value={s.max_failures}
				/>
			</div>
			<div>
				<label for="g-win">{t('guard.window')}</label>
				<input
					id="g-win"
					class="input"
					type="number"
					min="1"
					max="1440"
					bind:value={s.window_minutes}
				/>
			</div>
			<div>
				<label for="g-ban">{t('guard.banMinutes')}</label>
				<input
					id="g-ban"
					class="input"
					type="number"
					min="1"
					max="525600"
					bind:value={s.ban_minutes}
				/>
			</div>
		</div>
		<div>
			<label for="g-trusted">{t('guard.trusted')}</label>
			<textarea
				id="g-trusted"
				class="input font-mono"
				rows="3"
				placeholder="192.168.1.0/24"
				bind:value={trusted}></textarea>
			<p class="hint">{t('guard.trustedHint')}</p>
		</div>
		<div class="flex justify-end">
			<button class="btn btn-primary" type="submit">{t('common.save')}</button>
		</div>
		<h3 class="font-medium">{t('guard.bans')}</h3>
		{#if bans.length}
			<ul class="divide-y divide-slate-100 dark:divide-slate-800">
				{#each bans as ban (ban.ip)}
					<li class="flex items-center justify-between gap-3 py-2">
						<div class="min-w-0">
							<span class="font-mono text-sm">{ban.ip}</span>
							<span class="block text-xs text-slate-500 dark:text-slate-400"
								>{t('guard.banInfo', {
									user: ban.last_user || '—',
									until: formatDateTime(ban.banned_until)
								})}</span
							>
						</div>
						<button class="btn btn-sm" type="button" onclick={() => unban(ban)}
							>{t('guard.unban')}</button
						>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="hint">{t('guard.noBans')}</p>
		{/if}
	</form>
{/if}
