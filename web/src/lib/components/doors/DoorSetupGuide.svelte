<script lang="ts">
	import Tabs from '#lib/components/Tabs.svelte';
	import GuideSectionView from '#lib/components/doors/GuideSectionView.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { GuideLocation } from '#lib/doors/guide.ts';
	import { commonSections, vendorGuides } from '#lib/doors/vendors.ts';

	// Step-by-step setup per vendor and device type; the sections for all
	// door stations follow the device sections.
	let {
		location: loc,
		onselect
	}: {
		location: GuideLocation;
		onselect: (loc: GuideLocation) => void;
	} = $props();

	const vendor = $derived(vendorGuides.find((v) => v.id === loc.vendor) ?? vendorGuides[0]);
	const device = $derived(vendor.devices.find((d) => d.id === loc.device) ?? vendor.devices[0]);
	// SIP domain of TalkOps (`fsxml::SIP_DOMAIN`); the server accepts any domain.
	const params = $derived({ host: location.hostname, port: 5060, domain: 'talkops.local' });

	function selectVendor(id: string) {
		const v = vendorGuides.find((x) => x.id === id) ?? vendorGuides[0];
		onselect({ vendor: v.id, device: v.devices[0].id });
	}
</script>

<div class="space-y-4">
	<p class="hint">{t('doorGuide.intro')}</p>
	<div class="card space-y-3">
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<span class="mb-1 block text-sm font-medium">{t('doorGuide.vendor')}</span>
				<Tabs
					variant="pills"
					label={t('doorGuide.vendor')}
					tabs={vendorGuides.map((v) => ({ id: v.id, label: v.name }))}
					value={vendor.id}
					onchange={selectVendor}
				/>
			</div>
			<div>
				<span class="mb-1 block text-sm font-medium">{t('doorGuide.device')}</span>
				<Tabs
					variant="pills"
					label={t('doorGuide.device')}
					tabs={vendor.devices.map((d) => ({ id: d.id, label: t(d.label) }))}
					value={device.id}
					onchange={(id) => onselect({ vendor: vendor.id, device: id })}
				/>
			</div>
		</div>
		<p class="text-sm">{t(device.summary)}</p>
		<p class="rounded-lg bg-slate-100 p-2 text-xs break-words dark:bg-slate-800">
			{t('doorGuide.values', params)}
		</p>
	</div>

	{#key `${vendor.id}/${device.id}`}
		<div class="space-y-3">
			{#each device.sections as section, i (section.id)}
				<GuideSectionView {section} {params} open={i < 2} />
			{/each}
		</div>
	{/key}

	<h2>{t('doorGuide.commonHeading')}</h2>
	<div class="space-y-3">
		{#each commonSections as section (section.id)}
			<GuideSectionView {section} {params} />
		{/each}
	</div>
</div>
