import { randomInt } from 'node:crypto';
import { error, fail } from '@sveltejs/kit';
import * as repo from '#lib/server/repo.ts';
import { bool, int, mutate, optInt, str } from '#lib/server/forms.ts';
import { readExported } from '#lib/server/scripts.ts';
import type { ExportedTournament } from '#lib/exported.ts';
import type { Actions, PageServerLoad } from './$types';

function context(slug: string, numberParam: string) {
	const t = repo.getTournament(slug);
	if (!t) error(404, 'Tournament not found');
	const number = Number(numberParam);
	const position = repo.positions(t.id).find((p) => p.position === number);
	if (!position) error(404, 'This round has no category yet');
	return { t, number, category: position.category_slug };
}

export const load: PageServerLoad = async ({ params }) => {
	const { t, number, category } = context(params.slug, params.number);
	const exported = (
		await readExported<{ tournament: ExportedTournament }>(`tournaments/${t.slug}.json`)
	)?.tournament;
	const positions = repo.positions(t.id);
	return {
		tournament: t,
		number,
		category: repo.listCategories().find((c) => c.slug === category) ?? {
			slug: category,
			name: category,
			icon: null
		},
		lastPosition: positions.at(-1)?.position ?? number,
		round: repo.round(t.id, number) ?? null,
		participants: repo.participants(t.id),
		results: repo.results(t.id, number),
		nominations: repo.nominations(t.id, number),
		games: repo.listGames(),
		points: repo.points(t.id),
		usage: repo.tokenUsage(t.id, number),
		previousWinner: exported?.games.find((g) => g.number === number - 1)?.winner ?? null,
		outcome: exported?.games.find((g) => g.number === number) ?? null,
		champion: exported?.champion ?? null,
		standings: exported?.standings ?? []
	};
};

export const actions: Actions = {
	nominate: async ({ request, params }) => {
		const { t, number } = context(params.slug, params.number);
		const form = await request.formData();
		return mutate('Nomination added.', () =>
			repo.addNomination(t.id, number, {
				game_id: int(form, 'game'),
				source: str(form, 'source') as repo.Nomination['source'],
				player_id: optInt(form, 'player'),
				entries: optInt(form, 'entries') ?? 1
			})
		);
	},
	unnominate: async ({ request, params }) => {
		const { t } = context(params.slug, params.number);
		const form = await request.formData();
		return mutate('Nomination removed.', () => repo.removeNomination(t.id, int(form, 'id')));
	},
	setGame: async ({ request, params }) => {
		const { t, number } = context(params.slug, params.number);
		const form = await request.formData();
		const game = optInt(form, 'game');
		return mutate(game ? 'Game set.' : 'Game cleared.', () =>
			repo.setRoundGame(t.id, number, game)
		);
	},
	draw: async ({ params }) => {
		const { t, number } = context(params.slug, params.number);
		const noms = repo.nominations(t.id, number);
		if (!noms.length) return fail(400, { error: 'Add nominations before drawing.' });
		// Weighted by entries; each nomination is one ticket per entry.
		const tickets = noms.flatMap((n) => Array.from({ length: n.entries }, () => n.game_id));
		const picked = tickets[randomInt(tickets.length)];
		const name = repo.listGames().find((g) => g.id === picked)?.name ?? 'Game';
		return mutate(`🎲 Drawn: ${name}`, () => repo.setRoundGame(t.id, number, picked));
	},
	results: async ({ request, params }) => {
		const { t, number } = context(params.slug, params.number);
		const form = await request.formData();
		const finish = form.get('intent') === 'finish';
		return mutate(finish ? 'Game finished — standings updated.' : 'Draft saved.', () => {
			const rows = form.getAll('player').map((raw) => {
				const id = Number(raw);
				return {
					player_id: id,
					placement: int(form, `placement_${id}`),
					base_points: int(form, `points_${id}`),
					double_down: bool(form, `dd_${id}`),
					nemesis_player_id: optInt(form, `nemesis_${id}`)
				};
			});
			repo.saveResults(t.id, number, rows, finish);
		});
	},
	reopen: async ({ params }) => {
		const { t, number } = context(params.slug, params.number);
		return mutate('Game reopened — you can edit the results again.', () =>
			repo.reopenRound(t.id, number)
		);
	}
};
