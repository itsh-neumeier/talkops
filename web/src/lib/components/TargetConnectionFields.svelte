<script lang="ts">
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import {
		defaultConfig,
		s3Providers,
		targetKinds,
		webdavVendors,
		type TargetConfig,
		type TargetKind,
		type TargetSecret
	} from '#lib/backupTargets.ts';

	let {
		config = $bindable(),
		secret = $bindable(),
		allowInsecure = $bindable(),
		hasSecret = false,
		roots = [],
		idPrefix
	}: {
		config: TargetConfig;
		secret: TargetSecret;
		allowInsecure: boolean;
		/** A stored secret exists: empty fields keep it. */
		hasSecret?: boolean;
		roots?: string[];
		idPrefix: string;
	} = $props();

	const kindLabels: Record<TargetKind, MessageKey> = {
		local: 'targets.kind.local',
		smb: 'targets.kind.smb',
		s3: 'targets.kind.s3',
		ftp: 'targets.kind.ftp',
		webdav: 'targets.kind.webdav',
		sftp: 'targets.kind.sftp'
	};

	function setKind(kind: TargetKind) {
		if (kind !== config.kind) config = defaultConfig(kind);
	}
	const secretHint = $derived(hasSecret ? t('targets.secretKept') : '');
	const insecureNeeded = $derived(
		(config.kind === 'ftp' && config.tls === 'none') ||
			(config.kind === 'webdav' && config.url.startsWith('http://')) ||
			(config.kind === 's3' && config.endpoint.startsWith('http://'))
	);
	const id = (name: string) => `${idPrefix}-${name}`;
</script>

<div class="space-y-3">
	<div>
		<label for={id('kind')}>{t('targets.kind')}</label>
		<select
			id={id('kind')}
			class="input"
			value={config.kind}
			onchange={(e) => setKind(e.currentTarget.value as TargetKind)}
		>
			{#each targetKinds as kind (kind)}
				<option value={kind}>{t(kindLabels[kind])}</option>
			{/each}
		</select>
	</div>

	{#if config.kind === 'local'}
		<p class="hint">
			{t('targets.localHint')}
			{#if roots.length}{t('targets.localRoots')}
				<span class="font-mono">{roots.join(', ')}</span>{/if}
		</p>
	{/if}

	{#if config.kind === 'smb' || config.kind === 'ftp' || config.kind === 'sftp'}
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-3">
			<div class="@lg:col-span-2">
				<label for={id('host')}>{t('targets.host')}</label>
				<input
					id={id('host')}
					class="input"
					bind:value={config.host}
					required
					autocomplete="off"
					placeholder="nas.local"
				/>
			</div>
			<div>
				<label for={id('port')}>{t('targets.port')}</label>
				<input
					id={id('port')}
					class="input"
					type="number"
					min="1"
					max="65535"
					bind:value={config.port}
					placeholder={config.kind === 'smb' ? '445' : config.kind === 'sftp' ? '22' : '21'}
				/>
			</div>
		</div>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<label for={id('user')}>{t('targets.user')}</label>
				<input
					id={id('user')}
					class="input"
					bind:value={config.user}
					autocomplete="off"
					required={config.kind === 'sftp'}
				/>
			</div>
			<div>
				<label for={id('password')}>{t('targets.password')}</label>
				<input
					id={id('password')}
					class="input"
					type="password"
					bind:value={secret.password}
					autocomplete="new-password"
					placeholder={secretHint}
				/>
			</div>
		</div>
	{/if}

	{#if config.kind === 'smb'}
		<div>
			<label for={id('domain')}>{t('targets.domain')}</label>
			<input id={id('domain')} class="input" bind:value={config.domain} placeholder="WORKGROUP" />
		</div>
	{/if}

	{#if config.kind === 'ftp'}
		<div>
			<label for={id('tls')}>{t('targets.tls')}</label>
			<select id={id('tls')} class="input" bind:value={config.tls}>
				<option value="explicit">{t('targets.tls.explicit')}</option>
				<option value="implicit">{t('targets.tls.implicit')}</option>
				<option value="none">{t('targets.tls.none')}</option>
			</select>
		</div>
	{/if}

	{#if config.kind === 'sftp'}
		<div>
			<label for={id('key')}>{t('targets.privateKey')}</label>
			<textarea
				id={id('key')}
				class="input font-mono text-xs"
				rows="3"
				bind:value={secret.private_key}
				placeholder={hasSecret ? t('targets.secretKept') : '-----BEGIN OPENSSH PRIVATE KEY-----'}
			></textarea>
			<p class="hint">{t('targets.privateKeyHint')}</p>
		</div>
		<div>
			<span class="block text-sm font-medium">{t('targets.hostKey')}</span>
			{#if config.host_keys}
				<p class="font-mono text-xs break-all">{config.host_keys}</p>
				<button
					type="button"
					class="btn btn-sm mt-1"
					onclick={() => config.kind === 'sftp' && (config.host_keys = '')}
					>{t('targets.hostKeyReset')}</button
				>
			{:else}
				<p class="hint">{t('targets.hostKeyHint')}</p>
			{/if}
		</div>
	{/if}

	{#if config.kind === 's3'}
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<label for={id('provider')}>{t('targets.provider')}</label>
				<select id={id('provider')} class="input" bind:value={config.provider}>
					{#each s3Providers as p (p)}<option value={p}>{p}</option>{/each}
				</select>
			</div>
			<div>
				<label for={id('region')}>{t('targets.region')}</label>
				<input
					id={id('region')}
					class="input"
					bind:value={config.region}
					placeholder="eu-central-1"
				/>
			</div>
		</div>
		<div>
			<label for={id('endpoint')}>{t('targets.endpoint')}</label>
			<input
				id={id('endpoint')}
				class="input"
				bind:value={config.endpoint}
				required={config.provider !== 'AWS'}
				placeholder="https://s3.example.com"
			/>
			<p class="hint">{t('targets.endpointHint')}</p>
		</div>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<label for={id('access')}>{t('targets.accessKey')}</label>
				<input
					id={id('access')}
					class="input"
					bind:value={config.access_key_id}
					required
					autocomplete="off"
				/>
			</div>
			<div>
				<label for={id('secretkey')}>{t('targets.secretKey')}</label>
				<input
					id={id('secretkey')}
					class="input"
					type="password"
					bind:value={secret.secret_access_key}
					required={!hasSecret}
					autocomplete="new-password"
					placeholder={secretHint}
				/>
			</div>
		</div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={config.path_style} />
			{t('targets.pathStyle')}</label
		>
	{/if}

	{#if config.kind === 'webdav'}
		<div>
			<label for={id('url')}>{t('targets.url')}</label>
			<input
				id={id('url')}
				class="input"
				type="url"
				bind:value={config.url}
				required
				placeholder="https://cloud.example.com/remote.php/dav/files/USER"
			/>
		</div>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-3">
			<div>
				<label for={id('vendor')}>{t('targets.vendor')}</label>
				<select id={id('vendor')} class="input" bind:value={config.vendor}>
					{#each webdavVendors as v (v)}<option value={v}>{v}</option>{/each}
				</select>
			</div>
			<div>
				<label for={id('user')}>{t('targets.user')}</label>
				<input id={id('user')} class="input" bind:value={config.user} autocomplete="off" />
			</div>
			<div>
				<label for={id('password')}>{t('targets.password')}</label>
				<input
					id={id('password')}
					class="input"
					type="password"
					bind:value={secret.password}
					autocomplete="new-password"
					placeholder={secretHint}
				/>
			</div>
		</div>
		<p class="hint">{t('targets.webdavHint')}</p>
	{/if}

	{#if config.kind !== 'local' && config.kind !== 'smb' && config.kind !== 'sftp'}
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={allowInsecure} />
			{t('targets.insecure')}</label
		>
		{#if insecureNeeded && !allowInsecure}<p class="hint text-amber-700 dark:text-amber-400">
				{t('targets.insecureNeeded')}
			</p>{/if}
	{/if}
</div>
