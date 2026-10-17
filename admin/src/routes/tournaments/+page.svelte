<script lang="ts">
	import { enhance } from '$app/forms';
	import { resolve } from '$app/paths';
	import { statusLabel } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();
</script>

<h1>Tournaments</h1>
<Feedback {form} />

<section class="panel">
	<h2>Tournaments</h2>
	{#if data.tournaments.length}
		<table>
			<thead
				><tr><th>Name</th><th>Date</th><th>Status</th><th>Players</th><th>Rounds</th><th></th></tr
				></thead
			>
			<tbody>
				{#each data.tournaments as t (t.id)}
					<tr>
						<td
							><a href={resolve('/tournaments/[slug]', { slug: t.slug })}
								><strong>{t.name}</strong></a
							></td
						>
						<td class="num">{t.start_date ?? '–'}</td>
						<td><span class="badge {t.status}">{statusLabel[t.status]}</span></td>
						<td class="num">{t.players}</td>
						<td class="num">{t.rounds_done}/{t.positions}</td>
						<td><a class="btn" href={resolve('/tournaments/[slug]', { slug: t.slug })}>Open</a></td>
					</tr>
				{/each}
			</tbody>
		</table>
	{:else}
		<p class="muted">No tournaments yet. Create the first one below.</p>
	{/if}
</section>

<section class="panel">
	<h2>New tournament</h2>
	<form method="POST" action="?/create" use:enhance class="stack">
		<div class="row">
			<label>Name <input name="name" required placeholder="LAN 2026" /></label>
			<label>Slug <input name="slug" placeholder="auto from name" /></label>
			<label>Edition <input name="edition" placeholder="optional" /></label>
			<label>Start date <input name="start_date" type="date" /></label>
			<label>Location <input name="location" /></label>
		</div>
		<div class="row">
			<label
				>Challenger threshold <input
					name="challenger_threshold"
					type="number"
					min="1"
					required
					value="30"
				/></label
			>
			<label
				>Double Down tokens <input
					name="double_down_tokens"
					type="number"
					min="0"
					value="1"
				/></label
			>
			<label>Nominate tokens <input name="nominate_tokens" type="number" min="0" value="1" /></label
			>
			<label
				>Points per placement <input
					name="points"
					required
					value="6, 4, 3, 2, 1, 0"
					size="18"
				/></label
			>
			{#if data.tournaments.length}
				<label
					>Copy players from
					<select name="copy_players_from">
						<option value="">— nobody —</option>
						{#each data.tournaments as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
					</select>
				</label>
			{/if}
		</div>
		<p class="hint">
			Points per placement prefill base points when entering results (1st, 2nd, 3rd…). You can still
			adjust each result.
		</p>
		<div><button class="primary">Create tournament</button></div>
	</form>
</section>
