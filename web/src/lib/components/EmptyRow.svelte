<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';

	/** Single table row shown when a loaded list has no entries. */
	let { cols, text }: { cols: number; text?: string } = $props();
</script>

<tr><td class="empty" colspan={cols}>{text ?? t('common.empty')}</td></tr>
