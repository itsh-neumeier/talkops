<script lang="ts">
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { api, type PhoneNumber, type Settings, type SmtpSettings } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import AudioPicker, { type AudioMode } from '#lib/components/AudioPicker.svelte';
	import AudioPlayer from '#lib/components/AudioPlayer.svelte';
	import { musicUrl, type MusicTrack } from '#lib/audio.ts';
	import BackupSettings from '#lib/components/BackupSettings.svelte';
	import IdentitySettings from '#lib/components/IdentitySettings.svelte';
	import SipGuardSettings from '#lib/components/SipGuardSettings.svelte';
	import SystemDiagnostics from '#lib/components/SystemDiagnostics.svelte';
	import TranscriptionApiSettings from '#lib/components/TranscriptionApiSettings.svelte';
	import TwoFactor from '#lib/components/TwoFactor.svelte';
	import VoicemailControl from '#lib/components/VoicemailControl.svelte';
	import SettingsNav, { sections, type Section } from '#lib/components/SettingsNav.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole, logout } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	function initialSection(): Section {
		const hash = page.url.hash.slice(1);
		const found = sections.find((s) => s.id === hash && (!s.admin || hasRole('admin')));
		return found?.id ?? (hasRole('admin') ? 'telephony' : 'account');
	}
	let section = $state<Section>(initialSection());
	function show(id: Section) {
		section = id;
		history.replaceState(history.state, '', `#${id}`);
	}

	let settings = $state<Settings | null>(null);
	let numbers = $state<PhoneNumber[]>([]);
	let emergency = $state('');
	let error = $state('');
	let saved = $state(false);
	let recError = $state('');
	let recSaved = $state(false);
	let pw = $state({ current_password: '', new_password: '' });
	let pwError = $state('');
	let smtp = $state<SmtpSettings | null>(null);
	let smtpPassword = $state('');
	let smtpError = $state('');
	let smtpInfo = $state('');
	let testTo = $state('');
	let tracks = $state<MusicTrack[]>([]);
	/** '' = all pieces, a piece id, or 'own' for an own clip. */
	let music = $state('');
	let musicMode = $state<AudioMode>('upload');
	let musicPicker = $state<ReturnType<typeof AudioPicker>>();
	let musicError = $state('');
	let musicSaved = $state(false);
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
			music = settings.hold_music_clip_id ? 'own' : settings.hold_music;
			if (hasRole('admin')) tracks = await api.get<MusicTrack[]>('/audio/music');
			if (hasRole('operator')) numbers = await api.get<PhoneNumber[]>('/numbers');
			if (hasRole('admin')) smtp = await api.get<SmtpSettings>('/settings/smtp');
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

	async function saveRecording(e: SubmitEvent) {
		e.preventDefault();
		recError = '';
		recSaved = false;
		try {
			settings = await api.put<Settings>('/settings', {
				...settings,
				recording_retention_days: Number(settings!.recording_retention_days)
			});
			recSaved = true;
		} catch (err) {
			recError = errorMessage(err);
		}
	}

	async function saveMusic(e: SubmitEvent) {
		e.preventDefault();
		musicError = '';
		musicSaved = false;
		try {
			const own = music === 'own' ? ((await musicPicker?.ensure()) ?? null) : null;
			settings = await api.put<Settings>('/settings', {
				...settings,
				hold_music: music === 'own' ? '' : music,
				hold_music_clip_id: own
			});
			music = settings.hold_music_clip_id ? 'own' : settings.hold_music;
			musicSaved = true;
		} catch (err) {
			musicError = errorMessage(err);
		}
	}

	async function saveSmtp(e: SubmitEvent) {
		e.preventDefault();
		smtpError = '';
		smtpInfo = '';
		try {
			smtp = await api.put<SmtpSettings>('/settings/smtp', {
				...smtp,
				port: Number(smtp!.port),
				password: smtpPassword === '' ? null : smtpPassword
			});
			smtpPassword = '';
			smtpInfo = t('common.saved');
		} catch (err) {
			smtpError = errorMessage(err);
		}
	}

	async function testSmtp() {
		smtpError = '';
		smtpInfo = '';
		try {
			await api.post('/settings/smtp/test', { to: testTo });
			smtpInfo = t('smtp.testSent');
		} catch (err) {
			smtpError = errorMessage(err);
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
	<div class="flex flex-col gap-4 md:flex-row md:items-start">
		<SettingsNav {section} onsection={show} />
		<div class="min-w-0 flex-1 space-y-4">
			{#if section === 'telephony'}
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
							<label class="flex items-center gap-2"
								><input type="checkbox" bind:checked={settings.sip_uri_dialing} />
								{t('settings.sipUriDialing')}</label
							>
							<p class="hint">{t('settings.sipUriDialingHint')}</p>
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
				{:else if hasRole('admin') && !settings && !net.settled}
					<div class="card"><Skeleton lines={6} /></div>
				{/if}
			{:else if section === 'calls'}
				{#if settings && hasRole('admin')}
					<form class="card space-y-3" onsubmit={saveRecording}>
						<h2>{t('rec.title')}</h2>
						<p class="text-sm text-slate-600 dark:text-slate-300">{t('rec.hint')}</p>
						<ErrorBox error={recError} />
						{#if recSaved}<p class="text-sm text-emerald-700 dark:text-emerald-400">
								{t('common.saved')}
							</p>{/if}
						<fieldset class="space-y-1">
							<legend class="text-sm font-medium">{t('rec.record')}</legend>
							<label class="flex items-center gap-2"
								><input type="checkbox" bind:checked={settings.record_inbound} />
								{t('dir.inbound')}</label
							>
							<label class="flex items-center gap-2"
								><input type="checkbox" bind:checked={settings.record_outbound} />
								{t('dir.outbound')}</label
							>
							<label class="flex items-center gap-2"
								><input type="checkbox" bind:checked={settings.record_internal} />
								{t('dir.internal')}</label
							>
							<p class="hint">{t('rec.recordHint')}</p>
						</fieldset>
						<label class="flex items-center gap-2"
							><input type="checkbox" bind:checked={settings.recording_announcement} />
							{t('rec.announcement')}</label
						>
						<p class="hint">{t('rec.announcementHint')}</p>
						<div>
							<label for="rec-days">{t('rec.retention')}</label>
							<input
								id="rec-days"
								class="input w-32"
								type="number"
								min="0"
								max="3650"
								bind:value={settings.recording_retention_days}
							/>
							<p class="hint">{t('rec.retentionHint')}</p>
						</div>
						<label class="flex items-center gap-2"
							><input type="checkbox" bind:checked={settings.transcription_enabled} />
							{t('rec.transcription')}</label
						>
						<p class="hint">{t('rec.transcriptionHint')}</p>
						{#if settings.transcription_enabled}
							<div>
								<label for="s-tq">{t('stt.pass1')}</label>
								<select id="s-tq" class="input" bind:value={settings.transcription_quality}>
									<option value="fast">{t('rec.qualityFast')}</option>
									<option value="accurate">{t('rec.qualityAccurate')}</option>
									<option value="best">{t('rec.qualityBest')}</option>
									<option value="german">{t('rec.qualityGerman')}</option>
									<option value="api">{t('stt.engineApi')}</option>
								</select>
								<p class="hint">{t('stt.pass1Hint')}</p>
							</div>
							<div>
								<label for="s-tr">{t('stt.pass2')}</label>
								<select id="s-tr" class="input" bind:value={settings.transcription_refine}>
									<option value="">{t('stt.pass2None')}</option>
									<option value="accurate">{t('rec.qualityAccurate')}</option>
									<option value="best">{t('rec.qualityBest')}</option>
									<option value="german">{t('rec.qualityGerman')}</option>
									<option value="fast">{t('rec.qualityFast')}</option>
									<option value="api">{t('stt.engineApi')}</option>
								</select>
								<p class="hint">{t('stt.pass2Hint')}</p>
								{#if settings.transcription_refine === settings.transcription_quality}
									<p class="hint text-amber-700 dark:text-amber-400">{t('stt.pass2Same')}</p>
								{/if}
							</div>
							<p class="hint">{t('rec.qualityHint')}</p>
							<div>
								<label for="s-tv">{t('rec.vocabulary')}</label>
								<input
									id="s-tv"
									class="input"
									maxlength="600"
									bind:value={settings.transcription_vocabulary}
									placeholder={t('rec.vocabularyPlaceholder')}
								/>
								<p class="hint">{t('rec.vocabularyHint')}</p>
							</div>
							{#if settings.transcription_quality === 'api' || settings.transcription_refine === 'api'}
								<TranscriptionApiSettings />
							{/if}
						{/if}
						<div class="flex justify-end">
							<button class="btn btn-primary">{t('common.save')}</button>
						</div>
					</form>
				{:else if hasRole('admin') && !settings && !net.settled}
					<div class="card"><Skeleton lines={6} /></div>
				{/if}
				{#if settings && hasRole('admin')}
					<form class="card space-y-3" onsubmit={saveMusic}>
						<h2>{t('music.title')}</h2>
						<p class="text-sm text-slate-600 dark:text-slate-300">{t('music.hint')}</p>
						<ErrorBox error={musicError} />
						{#if musicSaved}<p class="text-sm text-emerald-700 dark:text-emerald-400">
								{t('common.saved')}
							</p>{/if}
						<div class="space-y-2" role="radiogroup" aria-label={t('music.title')}>
							<label class="flex items-center gap-2"
								><input type="radio" name="hold-music" value="" bind:group={music} />
								{t('music.all')}</label
							>
							{#each tracks as tr (tr.id)}
								<div class="space-y-1">
									<label class="flex items-center gap-2"
										><input
											type="radio"
											name="hold-music"
											value={tr.id}
											bind:group={music}
											disabled={!tr.available}
										/>
										{tr.title}</label
									>
									{#if tr.available && music === tr.id}
										<div class="pl-6"><AudioPlayer src={musicUrl(tr.id)} /></div>
									{/if}
								</div>
							{/each}
							{#if tracks.some((tr) => !tr.available)}
								<p class="hint">{t('music.unavailable')}</p>
							{/if}
							<label class="flex items-center gap-2"
								><input type="radio" name="hold-music" value="own" bind:group={music} />
								{t('music.own')}</label
							>
						</div>
						{#if music === 'own'}
							<AudioPicker
								bind:this={musicPicker}
								bind:mode={musicMode}
								bind:clipId={settings.hold_music_clip_id}
								modes={['upload', 'record', 'generate']}
								language={settings.default_language === 'en' ? 'en' : 'de'}
								hints={{ upload: t('queues.mohHint') }}
							/>
						{/if}
						<div class="flex justify-end">
							<button class="btn btn-primary">{t('common.save')}</button>
						</div>
					</form>
				{:else if hasRole('admin') && !settings && !net.settled}
					<div class="card"><Skeleton lines={6} /></div>
				{/if}
			{:else if section === 'voicemail'}
				{#if hasRole('admin')}<VoicemailControl />{/if}
			{:else if section === 'email'}
				{#if smtp && hasRole('admin')}
					<form class="card space-y-3" onsubmit={saveSmtp}>
						<h2>{t('smtp.title')}</h2>
						<p class="text-sm text-slate-600 dark:text-slate-300">{t('smtp.hint')}</p>
						<ErrorBox error={smtpError} />
						{#if smtpInfo}<p class="text-sm text-emerald-700 dark:text-emerald-400">
								{smtpInfo}
							</p>{/if}
						<div class="grid gap-3 sm:grid-cols-3">
							<div class="sm:col-span-2">
								<label for="smtp-host">{t('smtp.host')}</label>
								<input
									id="smtp-host"
									class="input font-mono"
									bind:value={smtp.host}
									placeholder="smtp.example.com"
								/>
							</div>
							<div>
								<label for="smtp-port">{t('smtp.port')}</label>
								<input
									id="smtp-port"
									class="input"
									type="number"
									min="1"
									max="65535"
									bind:value={smtp.port}
								/>
							</div>
							<div>
								<label for="smtp-sec">{t('smtp.security')}</label>
								<select id="smtp-sec" class="input" bind:value={smtp.security}>
									<option value="starttls">STARTTLS (587)</option>
									<option value="tls">TLS (465)</option>
									<option value="none">{t('smtp.none')}</option>
								</select>
							</div>
							<div>
								<label for="smtp-user">{t('smtp.username')}</label>
								<input id="smtp-user" class="input" autocomplete="off" bind:value={smtp.username} />
							</div>
							<div>
								<label for="smtp-pw">{t('login.password')}</label>
								<input
									id="smtp-pw"
									class="input"
									type="password"
									autocomplete="new-password"
									bind:value={smtpPassword}
									placeholder={smtp.has_password ? t('trunks.passwordKeep') : ''}
								/>
							</div>
						</div>
						<div>
							<label for="smtp-from">{t('smtp.from')}</label>
							<input
								id="smtp-from"
								class="input"
								bind:value={smtp.from}
								placeholder="TalkOps <pbx@example.com>"
							/>
						</div>
						<div class="flex flex-wrap items-end justify-between gap-2">
							<div class="flex items-end gap-2">
								<div>
									<label for="smtp-to">{t('smtp.testTo')}</label>
									<input id="smtp-to" class="input" type="email" bind:value={testTo} />
								</div>
								<button type="button" class="btn" disabled={!testTo} onclick={testSmtp}
									>{t('smtp.test')}</button
								>
							</div>
							<button class="btn btn-primary">{t('common.save')}</button>
						</div>
					</form>
				{/if}
			{:else if section === 'security'}
				{#if hasRole('admin')}<IdentitySettings />{/if}
				{#if hasRole('admin')}<SipGuardSettings />{/if}
			{:else if section === 'system'}
				{#if hasRole('admin')}
					<SystemDiagnostics />
					<BackupSettings />
				{/if}
			{:else}
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
				<TwoFactor />
			{/if}
		</div>
	</div>
</div>
