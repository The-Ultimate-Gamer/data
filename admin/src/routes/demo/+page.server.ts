import { fail } from '@sveltejs/kit';
import { demoPresent } from '#lib/server/repo.ts';
import { afterChange, demoData } from '#lib/server/scripts.ts';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = () => ({ present: demoPresent() });

async function run(action: 'add' | 'remove', message: string) {
	const result = await demoData(action);
	if (!result.ok) return fail(400, { error: result.output });
	const validation = await afterChange();
	return { message, errors: validation.errors, warnings: validation.warnings };
}

export const actions: Actions = {
	add: () => run('add', 'Demo data added. Publish to show it on the website.'),
	remove: () => run('remove', 'Demo data removed. Publish to take it off the website.')
};
