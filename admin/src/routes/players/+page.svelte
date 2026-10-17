<script lang="ts">
	import { enhance } from '$app/forms';
	import { keep } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();
</script>

<h1>Players</h1>
<Feedback {form} />

<section class="panel">
	<h2>Add player</h2>
	<form method="POST" action="?/create" use:enhance class="row">
		<label>Name <input name="name" required autocomplete="off" /></label>
		<label
			>Slug <input
				name="slug"
				placeholder="auto from name"
				pattern="[a-z0-9]+(-[a-z0-9]+)*"
			/></label
		>
		<label>Initials <input name="initials" maxlength="3" size="4" /></label>
		<label>Avatar URL <input name="avatar_url" type="url" placeholder="https://…" /></label>
		<button class="primary">Add</button>
	</form>
	<p class="hint">
		The slug becomes the player's permanent URL (/players/slug) and can't be changed here later.
	</p>
</section>

<section class="panel">
	<h2>All players ({data.players.length})</h2>
	{#if data.players.length}
		<div class="table-scroll">
			<table>
				<thead>
					<tr><th>Slug</th><th>Name</th><th>Initials</th><th>Avatar URL</th><th></th></tr>
				</thead>
				<tbody>
					{#each data.players as p (p.id)}
						<tr>
							<td class="num subtle">{p.slug}</td>
							<td colspan="4">
								<form method="POST" action="?/update" use:enhance={keep} class="row">
									<input type="hidden" name="id" value={p.id} />
									<input name="name" value={p.name} required aria-label="Name" />
									<input
										name="initials"
										value={p.initials ?? ''}
										maxlength="3"
										size="4"
										aria-label="Initials"
									/>
									<input
										name="avatar_url"
										type="url"
										value={p.avatar_url ?? ''}
										aria-label="Avatar URL"
										placeholder="https://…"
									/>
									<button class="small">Save</button>
									<button
										class="small danger"
										formaction="?/delete"
										onclick={(e) => {
											if (!confirm(`Delete ${p.name}?`)) e.preventDefault();
										}}>Delete</button
									>
								</form>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<p class="muted">No players yet.</p>
	{/if}
</section>
