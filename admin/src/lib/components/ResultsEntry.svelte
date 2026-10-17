<script lang="ts">
	import { enhance } from '$app/forms';
	import { keep } from '#lib/forms.ts';

	interface Participant {
		id: number;
		name: string;
	}

	interface Props {
		participants: Participant[];
		results: {
			player_id: number;
			placement: number;
			base_points: number;
			double_down: number;
			nemesis_player_id: number | null;
		}[];
		points: number[];
		doubleDownLeft: Record<number, number>;
		completed: boolean;
		hasGame: boolean;
	}

	let { participants, results, points, doubleDownLeft, completed, hasGame }: Props = $props();

	interface Row {
		id: number;
		name: string;
		included: boolean;
		placement: number | null;
		base: number | null;
		baseEdited: boolean;
		dd: boolean;
		nemesis: number | null;
	}

	function initialRows(): Row[] {
		return participants.map((p) => {
			const r = results.find((x) => x.player_id === p.id);
			return {
				id: p.id,
				name: p.name,
				included: results.length === 0 || !!r,
				placement: r?.placement ?? null,
				base: r?.base_points ?? null,
				baseEdited: !!r,
				dd: !!r?.double_down,
				nemesis: r?.nemesis_player_id ?? null
			};
		});
	}

	let rows = $state<Row[]>(initialRows());

	const active = $derived(rows.filter((r) => r.included));
	const nextPlacement = $derived(Math.max(0, ...active.map((r) => r.placement ?? 0)) + 1);
	const winners = $derived(active.filter((r) => r.placement === 1).length);
	const missing = $derived(active.filter((r) => r.placement === null || r.base === null).length);
	const canFinish = $derived(hasGame && active.length >= 2 && winners === 1 && missing === 0);

	function setPlacement(row: Row, placement: number | null) {
		row.placement = placement;
		if (!row.baseEdited) row.base = placement === null ? null : (points[placement - 1] ?? 0);
	}

	function tap(row: Row) {
		if (row.placement === null) setPlacement(row, nextPlacement);
		else if (row.placement === nextPlacement - 1) setPlacement(row, null);
	}

	function clearOrder() {
		for (const r of rows) {
			r.baseEdited = false;
			setPlacement(r, null);
		}
	}

	/** Preview only — the export script computes the authoritative points. */
	function preview(row: Row): number | null {
		if (row.placement === null || row.base === null) return null;
		const nemesis = active.find((r) => r.id === row.nemesis);
		const beat = nemesis?.placement != null && nemesis.placement > row.placement ? 1 : 0;
		return (row.base + beat) * (row.dd ? 2 : 1);
	}

	// Keep rows still while entering so taps don't land on a moving target; sort once finished.
	const ordered = $derived(
		completed
			? [...rows].sort(
					(a, b) =>
						Number(b.included) - Number(a.included) ||
						(a.placement ?? 999) - (b.placement ?? 999) ||
						a.name.localeCompare(b.name)
				)
			: rows
	);
</script>

<form method="POST" action="?/results" use:enhance={keep} class="stack">
	{#if !completed}
		<div class="row">
			<p class="hint grow">
				Tap players in finishing order (1st first). Tap the last one again to undo. Edit numbers
				directly for ties.
			</p>
			<button type="button" class="small" onclick={clearOrder}>Clear order</button>
		</div>
	{/if}

	<div class="table-scroll">
		<table>
			<thead>
				<tr>
					<th>Played</th>
					<th>Player</th>
					<th>Place</th>
					<th>Base pts</th>
					<th>Double Down</th>
					<th title="The Nemesis assigned before this game">Nemesis</th>
					<th>= Points</th>
				</tr>
			</thead>
			<tbody>
				{#each ordered as row (row.id)}
					{@const ddLeft = doubleDownLeft[row.id] ?? 0}
					<tr class:out={!row.included} class:first={row.placement === 1}>
						<td>
							<input
								type="checkbox"
								bind:checked={row.included}
								disabled={completed}
								aria-label="{row.name} played"
							/>
							{#if row.included}<input type="hidden" name="player" value={row.id} />{/if}
						</td>
						<td>
							<button
								type="button"
								class="tap"
								class:placed={row.placement !== null}
								disabled={completed || !row.included}
								onclick={() => tap(row)}
							>
								{row.name}
							</button>
						</td>
						<td>
							<input
								type="number"
								min="1"
								name="placement_{row.id}"
								value={row.placement ?? ''}
								oninput={(e) =>
									setPlacement(row, e.currentTarget.value ? Number(e.currentTarget.value) : null)}
								disabled={completed || !row.included}
								aria-label="{row.name} placement"
							/>
						</td>
						<td>
							<input
								type="number"
								min="0"
								name="points_{row.id}"
								value={row.base ?? ''}
								oninput={(e) => {
									row.baseEdited = true;
									row.base = e.currentTarget.value ? Number(e.currentTarget.value) : null;
								}}
								disabled={completed || !row.included}
								aria-label="{row.name} base points"
							/>
						</td>
						<td>
							<label class="inline">
								<input
									type="checkbox"
									name="dd_{row.id}"
									bind:checked={row.dd}
									disabled={completed || !row.included || (!row.dd && ddLeft <= 0)}
								/>
								<span class="subtle">{row.dd ? '2×' : `${ddLeft} left`}</span>
							</label>
						</td>
						<td>
							<select
								name="nemesis_{row.id}"
								bind:value={row.nemesis}
								disabled={completed || !row.included}
								aria-label="{row.name}'s Nemesis"
							>
								<option value={null}>—</option>
								{#each rows.filter((o) => o.id !== row.id) as o (o.id)}
									<option value={o.id}>{o.name}</option>
								{/each}
							</select>
						</td>
						<td class="num total">{row.included ? (preview(row) ?? '–') : ''}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	{#if !completed}
		<div class="row">
			<button name="intent" value="draft">Save draft</button>
			<button class="primary big" name="intent" value="finish" disabled={!canFinish}
				>✓ Finish game</button
			>
			<span class="hint">
				{#if !hasGame}Choose the game first.
				{:else if winners !== 1}Exactly one player must be 1st.
				{:else if missing}{missing} player(s) missing a place or points.
				{:else}Finishing updates standings, Challengers and the champion.{/if}
			</span>
		</div>
	{/if}
</form>

<style>
	.grow {
		flex: 1;
	}

	.tap {
		min-width: 8rem;
		justify-content: flex-start;
		font-weight: 700;
	}

	.tap.placed {
		border-color: var(--c-gold-line);
		background: var(--c-gold-soft);
	}

	tr.first .tap {
		background: var(--c-gold);
		color: var(--c-on-gold);
	}

	tr.out td {
		opacity: 0.45;
	}

	.total {
		font-weight: 800;
		font-size: var(--fs-lg);
	}

	.big {
		padding: 0.8rem 1.4rem;
		font-size: var(--fs-md);
	}
</style>
