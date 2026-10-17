<script lang="ts">
	interface Props {
		form:
			| { error?: string; message?: string; errors?: string[]; warnings?: string[] }
			| null
			| undefined;
	}

	let { form }: Props = $props();
</script>

{#if form?.error}
	<div class="feedback error" role="alert">{form.error}</div>
{:else if form?.message}
	<div class="feedback ok" role="status">
		<strong>{form.message}</strong>
		{#if form.errors?.length}
			<p>Validation found problems — the JSON was exported but fix these before publishing:</p>
			<ul class="errors">
				{#each form.errors as e, i (i)}<li>{e}</li>{/each}
			</ul>
		{/if}
		{#if form.warnings?.length}
			<ul class="warnings">
				{#each form.warnings as w, i (i)}<li>{w}</li>{/each}
			</ul>
		{/if}
	</div>
{/if}

<style>
	.feedback {
		padding: var(--sp-3) var(--sp-4);
		border-radius: var(--radius-md);
		font-size: var(--fs-sm);
		display: grid;
		gap: var(--sp-2);
	}

	.ok {
		border: 1px solid rgb(61 220 132 / 0.4);
		background: var(--c-live-soft);
	}

	.error {
		border: 1px solid rgb(255 84 104 / 0.45);
		background: var(--c-danger-soft);
	}

	ul {
		margin: 0;
		padding-left: 1.2rem;
	}

	.errors {
		color: var(--c-danger);
	}

	.warnings {
		color: var(--c-gold-bright);
	}
</style>
