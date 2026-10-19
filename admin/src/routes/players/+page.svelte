<script lang="ts">
	import { enhance } from '$app/forms';
	import { keep } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();

	let editing = $state<number | null>(null);
</script>

<h1>Players</h1>
<p class="intro muted">
	Everyone who has ever played. Add a player once and reuse them in every tournament.
</p>
<Feedback {form} />

<section class="panel">
	<h2>Add a player</h2>
	<form method="POST" action="?/create" use:enhance class="stack">
		<div class="row">
			<label>Name <input name="name" required autocomplete="off" placeholder="e.g. Alex" /></label>
			<button class="primary">Add player</button>
		</div>
		<details>
			<summary class="subtle">More options (web address, initials, picture)</summary>
			<div class="row options">
				<label
					>Web address <input
						name="slug"
						placeholder="made from the name"
						pattern="[a-z0-9]+(-[a-z0-9]+)*"
					/></label
				>
				<label>Initials <input name="initials" maxlength="3" size="4" placeholder="auto" /></label>
				<label>Picture URL <input name="avatar_url" type="url" placeholder="https://…" /></label>
			</div>
			<p class="hint">
				Pictures last longest when saved in the data repository as <code
					>avatars/&lt;web address&gt;.jpg</code
				>: they are published with the data and win over the URL. Discord links expire after about a
				day.
			</p>
		</details>
	</form>
</section>

<section class="panel">
	<h2>All players ({data.players.length})</h2>
	{#if data.players.length}
		<table>
			<thead><tr><th>Name</th><th>Web address</th><th></th></tr></thead>
			<tbody>
				{#each data.players as p (p.id)}
					<tr>
						<td>
							<strong>{p.name}</strong>
							{#if p.is_demo}<span class="badge">Demo</span>{/if}
						</td>
						<td class="num subtle">/players/{p.slug}</td>
						<td class="right">
							<button class="small" onclick={() => (editing = editing === p.id ? null : p.id)}>
								{editing === p.id ? 'Close' : 'Edit'}
							</button>
						</td>
					</tr>
					{#if editing === p.id}
						<tr>
							<td colspan="3">
								<form method="POST" action="?/update" use:enhance={keep} class="stack edit">
									<input type="hidden" name="id" value={p.id} />
									<div class="row">
										<label>Name <input name="name" value={p.name} required /></label>
										<label
											>Initials <input
												name="initials"
												value={p.initials ?? ''}
												maxlength="3"
												size="4"
											/></label
										>
										<label
											>Picture URL <input
												name="avatar_url"
												type="url"
												value={p.avatar_url ?? ''}
												placeholder="https://…"
											/></label
										>
									</div>
									<label
										>Web address
										<input name="slug" value={p.slug} required pattern="[a-z0-9]+(-[a-z0-9]+)*" />
									</label>
									<p class="hint">
										The player’s page is at <code>/players/{p.slug}</code>. You can change it — the
										website updates its own links — but links people already shared will stop
										working.
									</p>
									<div class="row">
										<button class="primary">Save</button>
										<button
											class="danger"
											formaction="?/delete"
											onclick={(e) => {
												if (
													!confirm(`Delete ${p.name}? This only works if they have never played.`)
												)
													e.preventDefault();
											}}>Delete player</button
										>
									</div>
								</form>
							</td>
						</tr>
					{/if}
				{/each}
			</tbody>
		</table>
	{:else}
		<p class="muted">No players yet — add the first one above.</p>
	{/if}
</section>

<style>
	.intro {
		max-width: 65ch;
	}

	.options {
		margin-top: var(--sp-3);
	}

	summary {
		cursor: pointer;
	}

	.right {
		text-align: right;
	}

	.edit {
		padding: var(--sp-3);
		background: var(--c-bg-raised);
		border-radius: var(--radius-md);
	}
</style>
