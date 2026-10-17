<script lang="ts">
	import { ordinal } from '#lib/forms.ts';
	import type { ExportedStanding } from '#lib/exported.ts';

	interface Props {
		standings: ExportedStanding[];
		highlight?: string[];
	}

	let { standings, highlight = [] }: Props = $props();
</script>

{#if standings.length}
	<div class="table-scroll">
		<table>
			<thead>
				<tr
					><th>#</th><th>Player</th><th>Points</th><th>Status</th><th>Wins</th><th>Tokens left</th
					></tr
				>
			</thead>
			<tbody>
				{#each standings as s (s.player.slug)}
					<tr class:hl={highlight.includes(s.player.slug)}>
						<td class="num">{ordinal(s.rank)}</td>
						<td><strong>{s.player.name}</strong></td>
						<td class="num pts">{s.points}</td>
						<td>
							{#if s.isChampion}<span class="badge gold">👑 Ultimate Gamer</span>
							{:else if s.isChallenger}<span class="badge challenger"
									>⚔ Challenger · since #{s.challengerSinceGame}</span
								>
							{:else}<span class="subtle">Contender</span>{/if}
						</td>
						<td class="num">{s.gamesWon}</td>
						<td class="num">DD×{s.tokens.doubleDown} · N×{s.tokens.nominate}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{:else}
	<p class="muted">No standings yet — they appear after players are registered.</p>
{/if}

<style>
	.pts {
		font-weight: 700;
		font-size: var(--fs-md);
	}

	tr.hl td {
		background: var(--c-gold-soft);
	}
</style>
