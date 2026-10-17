<script lang="ts">
	import { enhance } from '$app/forms';
	import { keep } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();

	let filter = $state('');
	let editing = $state<number | null>(null);

	const shown = $derived(
		data.games.filter(
			(g) =>
				!filter ||
				g.name.toLowerCase().includes(filter.toLowerCase()) ||
				g.categories.includes(filter.toLowerCase())
		)
	);
	const catName = $derived(
		Object.fromEntries(data.categories.map((c) => [c.slug, `${c.icon ?? ''} ${c.name}`]))
	);
</script>

<h1>Games</h1>
<p class="intro muted">
	The library of games that can be nominated. Tag each game with the categories it fits, so it shows
	up when that category is played.
</p>
<Feedback {form} />

{#snippet gameForm(g?: (typeof data.games)[number])}
	<form method="POST" action="?/save" use:enhance={g ? keep : undefined} class="stack">
		{#if g}<input type="hidden" name="id" value={g.id} />{/if}
		<div class="row">
			<label>Name <input name="name" value={g?.name ?? ''} required /></label>
			{#if g}
				<label
					>Web address <input
						name="slug"
						value={g.slug}
						required
						pattern="[a-z0-9]+(-[a-z0-9]+)*"
						title="Changing it breaks links people already shared"
					/></label
				>
			{:else}
				<label>Web address <input name="slug" placeholder="made from the name" /></label>
			{/if}
			<label
				>Min players <input
					name="min_players"
					type="number"
					min="1"
					value={g?.min_players ?? ''}
				/></label
			>
			<label
				>Max players <input
					name="max_players"
					type="number"
					min="1"
					value={g?.max_players ?? ''}
				/></label
			>
			<label class="inline"
				><input type="checkbox" name="available" checked={g ? !!g.available : true} /> Available for nomination</label
			>
		</div>
		<label>Description <input name="description" value={g?.description ?? ''} /></label>
		<fieldset>
			<legend>Categories</legend>
			<div class="cats">
				{#each data.categories as c (c.slug)}
					<label class="inline"
						><input
							type="checkbox"
							name="categories"
							value={c.slug}
							checked={g?.categories.includes(c.slug)}
						/>
						{c.icon}
						{c.name}</label
					>
				{/each}
			</div>
		</fieldset>
		<div class="row">
			<button class="primary">{g ? 'Save' : 'Add game'}</button>
			{#if g}
				<button type="button" onclick={() => (editing = null)}>Close</button>
				<button
					class="danger"
					formaction="?/delete"
					onclick={(e) => {
						if (!confirm(`Delete ${g.name}?`)) e.preventDefault();
					}}>Delete</button
				>
			{/if}
		</div>
	</form>
{/snippet}

<section class="panel">
	<h2>Add game</h2>
	{@render gameForm()}
</section>

<section class="panel">
	<div class="spread">
		<h2>Library ({data.games.length})</h2>
		<input
			type="search"
			placeholder="Filter by name or category"
			bind:value={filter}
			aria-label="Filter games"
		/>
	</div>
	<div class="table-scroll">
		<table>
			<thead
				><tr><th>Game</th><th>Categories</th><th>Players</th><th>Status</th><th></th></tr></thead
			>
			<tbody>
				{#each shown as g (g.id)}
					<tr>
						<td><strong>{g.name}</strong> <span class="subtle num">{g.slug}</span></td>
						<td>{g.categories.map((c) => catName[c] ?? c).join(', ') || '–'}</td>
						<td class="num">{g.min_players ?? '?'}–{g.max_players ?? '?'}</td>
						<td
							>{#if g.available}<span class="badge live">Available</span>{:else}<span class="badge"
									>Retired</span
								>{/if}</td
						>
						<td
							><button class="small" onclick={() => (editing = editing === g.id ? null : g.id)}
								>Edit</button
							></td
						>
					</tr>
					{#if editing === g.id}
						<tr><td colspan="5">{@render gameForm(g)}</td></tr>
					{/if}
				{/each}
			</tbody>
		</table>
	</div>
</section>

<style>
	.intro {
		max-width: 65ch;
	}

	.cats {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
		gap: var(--sp-2);
	}
</style>
