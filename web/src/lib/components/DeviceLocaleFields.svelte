<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	let {
		values = $bindable({}),
		prefix = 'locale',
		disabled = false,
		ciscoLocale = false
	}: {
		values: {
			language?: string;
			time_zone?: string;
			ntp_server?: string;
			vendor_settings?: Record<string, string>;
		};
		prefix?: string;
		disabled?: boolean;
		/** Cisco Enterprise phone: offer the uploaded Cisco user locales. */
		ciscoLocale?: boolean;
	} = $props();

	type CiscoLocale = { name: string; lang_code: string };
	let locales = $state<CiscoLocale[]>([]);
	onMount(async () => {
		if (!ciscoLocale) return;
		try {
			locales = await api.get<CiscoLocale[]>('/phone-locales/cisco');
		} catch {
			locales = [];
		}
	});
	function setCiscoLocale(name: string) {
		const vendor = { ...(values.vendor_settings ?? {}) };
		if (name) vendor.cisco_enterprise_locale = name;
		else delete vendor.cisco_enterprise_locale;
		values = { ...values, vendor_settings: vendor };
	}
</script>

<fieldset class="space-y-3">
	<legend>{t('prov.localeTitle')}</legend>
	<p class="hint">{t('prov.localeHint')}</p>
	<div class="grid grid-cols-1 gap-3 @lg:grid-cols-3">
		<label for="{prefix}-language"
			>{t('prov.language')}
			<select
				id="{prefix}-language"
				class="input"
				value={values.language ?? ''}
				{disabled}
				onchange={(e) => (values = { ...values, language: e.currentTarget.value })}
			>
				<option value="">{t('prov.inheritLocale')}</option><option value="de"
					>{t('prov.german')}</option
				><option value="en">{t('prov.english')}</option>
			</select>
		</label>
		<label for="{prefix}-zone"
			>{t('prov.timeZone')}
			<input
				id="{prefix}-zone"
				class="input"
				value={values.time_zone ?? ''}
				placeholder="Europe/Berlin"
				list="{prefix}-zones"
				{disabled}
				oninput={(e) => (values = { ...values, time_zone: e.currentTarget.value })}
			/>
			<datalist id="{prefix}-zones"
				>{#each ['Europe/Berlin', 'Europe/Vienna', 'Europe/Zurich', 'Europe/London', 'Europe/Paris', 'UTC'] as zone}<option
						value={zone}
					></option>{/each}</datalist
			>
		</label>
		<label for="{prefix}-ntp"
			>{t('prov.ntpServer')}
			<input
				id="{prefix}-ntp"
				class="input"
				value={values.ntp_server ?? ''}
				placeholder="pool.ntp.org"
				{disabled}
				oninput={(e) => (values = { ...values, ntp_server: e.currentTarget.value })}
			/>
		</label>
		{#if ciscoLocale}
			<label for="{prefix}-cisco-locale"
				>{t('prov.ciscoLocale')}
				<select
					id="{prefix}-cisco-locale"
					class="input"
					value={values.vendor_settings?.cisco_enterprise_locale ?? ''}
					{disabled}
					onchange={(e) => setCiscoLocale(e.currentTarget.value)}
				>
					<option value="">{t('prov.ciscoLocaleAuto')}</option>
					{#each locales as locale (locale.name)}<option value={locale.name}
							>{locale.name} ({locale.lang_code})</option
						>{/each}
				</select>
				{#if locales.length === 0}<span class="hint">{t('prov.ciscoLocaleNone')}</span>{/if}
			</label>
		{/if}
	</div>
</fieldset>
