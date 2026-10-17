import { error, redirect } from '@sveltejs/kit';
import * as repo from '#lib/server/repo.ts';
import { bool, int, mutate, optInt, optStr, parsePoints, str } from '#lib/server/forms.ts';
import { readExported } from '#lib/server/scripts.ts';
import type { ExportedTournament } from '#lib/exported.ts';
import type { Actions, PageServerLoad } from './$types';

function tournament(slug: string) {
	const t = repo.getTournament(slug);
	if (!t) error(404, 'Tournament not found');
	return t;
}

export const load: PageServerLoad = async ({ params }) => {
	const t = tournament(params.slug);
	const exported = await readExported<{ tournament: ExportedTournament }>(
		`tournaments/${t.slug}.json`
	);
	return {
		tournament: t,
		players: repo.listPlayers(),
		participantIds: repo.participants(t.id).map((p) => p.id),
		categories: repo.listCategories(),
		awards: repo.listAwards(),
		positions: repo.positions(t.id),
		rounds: repo.rounds(t.id),
		points: repo.points(t.id),
		grants: repo.grants(t.id),
		records: repo.records(t.id),
		exported: exported?.tournament ?? null
	};
};

export const actions: Actions = {
	settings: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		const slug = optStr(form, 'slug') ?? t.slug;
		const result = await mutate('Settings saved.', () =>
			repo.updateTournament(t.id, {
				name: str(form, 'name'),
				slug,
				edition: optStr(form, 'edition'),
				start_date: optStr(form, 'start_date'),
				end_date: optStr(form, 'end_date'),
				location: optStr(form, 'location'),
				status: str(form, 'status') as repo.Tournament['status'],
				challenger_threshold: int(form, 'challenger_threshold'),
				double_down_tokens: int(form, 'double_down_tokens'),
				nominate_tokens: int(form, 'nominate_tokens')
			})
		);
		if (slug !== t.slug && 'message' in result) redirect(303, `/tournaments/${slug}`);
		return result;
	},
	status: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		const status = str(form, 'status') as repo.Tournament['status'];
		return mutate(`Tournament marked ${status}.`, () =>
			repo.updateTournament(t.id, {
				...t,
				status,
				end_date:
					status === 'completed'
						? (t.end_date ?? new Date().toISOString().slice(0, 10))
						: t.end_date
			})
		);
	},
	participants: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Participants saved.', () =>
			repo.setParticipants(t.id, form.getAll('player').map(Number))
		);
	},
	addPosition: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Category added.', () => repo.addPosition(t.id, str(form, 'category')));
	},
	removePosition: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Category removed.', () => repo.removePosition(t.id, int(form, 'position')));
	},
	movePosition: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		const position = int(form, 'position');
		return mutate('Order changed.', () =>
			repo.swapPositions(t.id, position, position + (bool(form, 'up') ? -1 : 1))
		);
	},
	points: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Points table saved.', () =>
			repo.setPoints(t.id, parsePoints(str(form, 'points')))
		);
	},
	grant: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Award granted.', () =>
			repo.grantAward(t.id, str(form, 'award'), int(form, 'player'), optStr(form, 'detail'))
		);
	},
	revoke: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Award removed.', () => repo.revokeAward(t.id, int(form, 'id')));
	},
	addRecord: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Record added.', () =>
			repo.addRecord(t.id, str(form, 'label'), str(form, 'value'), optInt(form, 'player'))
		);
	},
	removeRecord: async ({ request, params }) => {
		const t = tournament(params.slug);
		const form = await request.formData();
		return mutate('Record removed.', () => repo.removeRecord(t.id, int(form, 'position')));
	}
};
