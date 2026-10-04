<script lang="ts">
	let { error }: { error: string } = $props();
</script>

{#if error}
	<p class="error-box" role="alert">{error}</p>
{/if}
