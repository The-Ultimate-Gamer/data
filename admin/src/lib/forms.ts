import type { SubmitFunction } from '$app/forms';

/** use:enhance that keeps form values after a successful submit (for edit forms). */
export const keep: SubmitFunction = () => {
	return async ({ update }) => update({ reset: false });
};

export const statusLabel = {
	upcoming: 'Upcoming',
	ongoing: 'Live',
	completed: 'Completed'
} as const;

export function ordinal(n: number): string {
	const mod100 = n % 100;
	if (mod100 >= 11 && mod100 <= 13) return `${n}th`;
	return `${n}${['th', 'st', 'nd', 'rd'][n % 10] ?? 'th'}`;
}
