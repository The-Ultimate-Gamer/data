import { fail } from '@sveltejs/kit';
import { config } from '#lib/server/config.ts';
import * as git from '#lib/server/git.ts';
import { afterChange } from '#lib/server/scripts.ts';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async () => {
	// Re-export first so the change list always reflects the latest database.
	const validation = await afterChange();
	const status = config.canPublish ? await git.status() : null;
	return {
		validation: { errors: validation.errors, warnings: validation.warnings },
		git: status,
		unpublished: status ? git.isUnpublished(status) : false,
		deployment: status?.head ? await git.deployment(status.head) : null,
		siteUrl: config.siteUrl
	};
};

function summary(changes: git.Change[]): string {
	const labels = changes.map((c) => c.label);
	if (!labels.length) return 'Update tournament data';
	const head = labels.slice(0, 4).join(', ');
	return `Update ${head}${labels.length > 4 ? ` and ${labels.length - 4} more` : ''}`;
}

export const actions: Actions = {
	publish: async ({ request }) => {
		if (!config.canPublish) {
			return fail(400, { error: 'Publishing is turned off while you use a practice database.' });
		}
		const validation = await afterChange();
		if (validation.errors.length) {
			return fail(400, { error: 'Some data needs fixing before it can be published (see below).' });
		}
		const before = await git.status();
		const form = await request.formData();
		const message = String(form.get('message') ?? '').trim() || summary(before.changes);
		try {
			await git.commit(message);
			await git.push();
		} catch (error) {
			const e = error as { stderr?: string; message: string };
			return fail(500, {
				error: `Your changes are saved, but sending them to GitHub failed. Check your internet connection and try again.\n\n${(e.stderr || e.message).trim()}`
			});
		}
		return {
			message: 'Sent to GitHub! The website is updating now — this page will show when it’s live.'
		};
	},
	check: async () => {
		const validation = await afterChange();
		return {
			message: validation.errors.length
				? 'Checked — some data needs fixing.'
				: 'Checked — everything looks good.'
		};
	}
};
