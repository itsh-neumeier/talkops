<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type PhoneNumber, type Settings } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole, logout } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let settings = $state<Settings | null>(null);
	let numbers = $state<PhoneNumber[]>([]);
	let emergency = $state('');
	let error = $state('');
	let saved = $state(false);
	let pw = $state({ current_password: '', new_password: '' });
	let pwError = $state('');
	// Zones with a matching Yealink time zone entry (talkops-provisioning).
	const timezones = [
		'Europe/Berlin',
		'Europe/Vienna',
		'Europe/Zurich',
		'Europe/Amsterdam',
		'Europe/Paris',
		'Europe/Rome',
		'Europe/Madrid',
		'Europe/London'
	];

	onMount(async () => {
		try {
			settings = await api.get<Settings>('/settings');
			emergency = settings.emergency_numbers.join(', ');
			if (hasRole('operator')) numbers = await api.get<PhoneNumber[]>('/numbers');
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		saved = false;
		try {
			settings = await api.put<Settings>('/settings', {
				...settings,
				emergency_numbers: emergency
					.split(',')
					.map((s) => s.trim())
					.filter(Boolean),
				default_number_id: settings!.default_number_id || null
			});
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function changePassword(e: SubmitEvent) {
		e.preventDefault();
		pwError = '';
		try {
			await api.post('/auth/password', pw);
			await logout();
		} catch (err) {
			pwError = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<h1>{t('nav.settings')}</h1>
	{#if settings && hasRole('admin')}
		<form class="card space-y-4" onsubmit={save}>
			<h2>{t('settings.dialing')}</h2>
			<ErrorBox {error} />
			{#if saved}<p class="text-sm text-emerald-700 dark:text-emerald-400">
					{t('common.saved')}
				</p>{/if}
			<div class="grid gap-3 sm:grid-cols-2">
				<div>
					<label for="s-cc">{t('settings.countryCode')}</label><input
						id="s-cc"
						class="input font-mono"
						bind:value={settings.country_code}
						required
					/>
				</div>
				<div>
					<label for="s-ac">{t('settings.areaCode')}</label>
					<input id="s-ac" class="input font-mono" bind:value={settings.area_code} />
					<p class="hint">{t('settings.areaHint')}</p>
				</div>
				<div>
					<label for="s-np">{t('settings.nationalPrefix')}</label><input
						id="s-np"
						class="input font-mono"
						bind:value={settings.national_prefix}
					/>
				</div>
				<div>
					<label for="s-ip">{t('settings.internationalPrefix')}</label><input
						id="s-ip"
						class="input font-mono"
						bind:value={settings.international_prefix}
						required
					/>
				</div>
			</div>
			<div>
				<label for="s-em">{t('settings.emergency')}</label>
				<input id="s-em" class="input font-mono" bind:value={emergency} required />
				<p class="hint">{t('settings.emergencyHint')}</p>
			</div>
			<div>
				<label for="s-def">{t('settings.defaultNumber')}</label>
				<select id="s-def" class="input" bind:value={settings.default_number_id}>
					<option value={null}>—</option>
					{#each numbers as n (n.id)}<option value={n.id}>{n.e164} {n.label}</option>{/each}
				</select>
				<p class="hint">{t('settings.defaultNumberHint')}</p>
			</div>
			<div>
				<label for="s-ext">{t('settings.externalIp')}</label>
				<input
					id="s-ext"
					class="input font-mono"
					bind:value={settings.external_ip}
					placeholder="203.0.113.10"
				/>
				<p class="hint">{t('settings.externalIpHint')}</p>
			</div>
			<div>
				<label for="s-tz">{t('settings.timezone')}</label>
				<select id="s-tz" class="input" bind:value={settings.timezone}>
					{#each timezones as tz (tz)}<option value={tz}>{tz}</option>{/each}
				</select>
				<p class="hint">{t('settings.timezoneHint')}</p>
			</div>
			<div class="flex justify-end">
				<button class="btn btn-primary">{t('common.save')}</button>
			</div>
		</form>
	{/if}

	<form class="card max-w-md space-y-3" onsubmit={changePassword}>
		<h2>{t('settings.account')}</h2>
		<ErrorBox error={pwError} />
		<div>
			<label for="p-cur">{t('settings.currentPassword')}</label><input
				id="p-cur"
				class="input"
				type="password"
				autocomplete="current-password"
				bind:value={pw.current_password}
				required
			/>
		</div>
		<div>
			<label for="p-new">{t('settings.newPassword')}</label>
			<input
				id="p-new"
				class="input"
				type="password"
				autocomplete="new-password"
				minlength="10"
				bind:value={pw.new_password}
				required
			/>
			<p class="hint">{t('setup.passwordHint')}</p>
		</div>
		<button class="btn">{t('settings.changePassword')}</button>
	</form>
</div>
