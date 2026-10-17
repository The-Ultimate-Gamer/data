import { fail } from '@sveltejs/kit';
import { config } from '#lib/server/config.ts';
import * as git from '#lib/server/git.ts';
import { afterChange, validate } from '#lib/server/scripts.ts';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async () => ({
	validation: await validate(),
	git: config.canPublish ? await git.status() : null,
	defaultMessage: `Results update ${new Date().toISOString().slice(0, 16).replace('T', ' ')}`
});

async function commitAndMaybePush(form: FormData, push: boolean) {
	if (!config.canPublish)
		return fail(400, { error: 'Publishing is disabled when using an alternate database.' });
	const validation = await afterChange();
	if (validation.errors.length) {
		return fail(400, {
			error: 'Fix the validation errors before publishing.',
			errors: validation.errors
		});
	}
	const message = String(form.get('message') ?? '').trim() || 'Results update';
	const before = await git.status();
	try {
		if (before.changes.length) await git.commit(message);
		if (push) await git.push();
	} catch (error) {
		return fail(500, {
			error: `Git failed: ${(error as { stderr?: string; message: string }).stderr || (error as Error).message}`
		});
	}
	if (!before.changes.length && !push)
		return { message: 'Nothing to commit — everything is already saved.' };
	return {
		message: push
			? 'Published! GitHub Actions is validating and deploying; the site updates within a few minutes.'
			: 'Committed locally. Push when you are ready to publish.'
	};
}

export const actions: Actions = {
	export: async () => {
		const validation = await afterChange();
		return {
			message: 'Exported and validated.',
			errors: validation.errors,
			warnings: validation.warnings
		};
	},
	commit: async ({ request }) => commitAndMaybePush(await request.formData(), false),
	publish: async ({ request }) => commitAndMaybePush(await request.formData(), true)
};
