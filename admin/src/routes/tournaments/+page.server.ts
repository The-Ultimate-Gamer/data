import { redirect } from '@sveltejs/kit';
import * as repo from '#lib/server/repo.ts';
import { int, mutate, optInt, optStr, parsePoints, str } from '#lib/server/forms.ts';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = () => ({ tournaments: repo.listTournaments() });

export const actions: Actions = {
	create: async ({ request }) => {
		const form = await request.formData();
		let slug = '';
		const result = await mutate('Tournament created.', () => {
			slug = repo.createTournament({
				name: str(form, 'name'),
				slug: optStr(form, 'slug') ?? undefined,
				edition: optStr(form, 'edition'),
				start_date: optStr(form, 'start_date'),
				end_date: null,
				location: optStr(form, 'location'),
				status: 'upcoming',
				challenger_threshold: int(form, 'challenger_threshold'),
				double_down_tokens: int(form, 'double_down_tokens'),
				nominate_tokens: int(form, 'nominate_tokens'),
				points: parsePoints(str(form, 'points')),
				copyPlayersFrom: optInt(form, 'copy_players_from')
			});
		});
		if (slug) redirect(303, `/tournaments/${slug}`);
		return result;
	}
};
