<script lang="ts">
	import type { PhoneSettingInfo } from '#lib/api.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';

	let {
		catalog,
		values = $bindable({}),
		family = null,
		inherited = null,
		idPrefix = 'ps'
	}: {
		catalog: PhoneSettingInfo[];
		/** Values as edited; an unset key is left alone. */
		values: Record<string, string>;
		/** Only settings this phone family takes (null: all). */
		family?: string | null;
		/** Values for all phones (on a single phone's page). */
		inherited?: Record<string, string> | null;
		idPrefix?: string;
	} = $props();

	const shown = $derived(catalog.filter((s) => !family || s.families.includes(family)));
	const groups = $derived([...new Set(shown.map((s) => s.group))]);
	const wifiOnly = (s: PhoneSettingInfo) => !s.families.includes('desk');
	/** All shown settings are for Wi-Fi handsets only: one hint, not one per field. */
	const allWifiOnly = $derived(!family && shown.length > 0 && shown.every(wifiOnly));

	function valueLabel(s: PhoneSettingInfo, v: string): string {
		if (s.kind === 'bool') return v === '1' ? t('phoneset.on') : t('phoneset.off');
		if (s.key === 'backlight_time') {
			const secs = Number(v);
			if (secs === 0) return t('phoneset.always');
			return secs < 60 ? `${secs} s` : `${secs / 60} min`;
		}
		const key = `phoneset.v.${s.key}.${v}` as MessageKey;
		return t(key) ?? v;
	}

	/** First option: keep what the phone (or "all phones") has. */
	function keepLabel(s: PhoneSettingInfo): string {
		const all = inherited?.[s.key];
		if (all !== undefined) return t('phoneset.inherit', { value: valueLabel(s, all) });
		if (s.default !== null) return t('phoneset.keepDefault', { value: valueLabel(s, s.default) });
		return t('phoneset.keep');
	}

	function set(key: string, v: string) {
		const next = { ...values };
		if (v === '') delete next[key];
		else next[key] = v;
		values = next;
	}
</script>

{#if allWifiOnly}
	<p class="hint">{t('phoneset.onlyWifi')}</p>
{/if}
{#each groups as group (group)}
	<fieldset class="space-y-2">
		<legend class="text-sm font-semibold">{t(`phoneset.group.${group}` as MessageKey)}</legend>
		<div class="grid gap-3 sm:grid-cols-2">
			{#each shown.filter((s) => s.group === group) as s (s.key)}
				<div>
					<label for="{idPrefix}-{s.key}">{t(`phoneset.${s.key}` as MessageKey)}</label>
					<select
						id="{idPrefix}-{s.key}"
						class="input"
						value={values[s.key] ?? ''}
						onchange={(e) => set(s.key, e.currentTarget.value)}
					>
						<option value="">{keepLabel(s)}</option>
						{#each s.values as v (v)}
							<option value={v}>{valueLabel(s, v)}</option>
						{/each}
					</select>
					{#if !family && !allWifiOnly && wifiOnly(s)}
						<p class="hint">{t('phoneset.onlyWifi')}</p>
					{/if}
				</div>
			{/each}
		</div>
	</fieldset>
{/each}
