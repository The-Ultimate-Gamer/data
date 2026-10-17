import { fail } from '@sveltejs/kit';
import { UserError } from './repo.ts';
import { afterChange } from './scripts.ts';

export function str(form: FormData, key: string): string {
	const value = String(form.get(key) ?? '').trim();
	if (!value) throw new UserError(`"${key}" is required.`);
	return value;
}

export function optStr(form: FormData, key: string): string | null {
	const value = String(form.get(key) ?? '').trim();
	return value || null;
}

export function int(form: FormData, key: string): number {
	const value = Number(form.get(key));
	if (!Number.isInteger(value)) throw new UserError(`"${key}" must be a whole number.`);
	return value;
}

export function optInt(form: FormData, key: string): number | null {
	const raw = String(form.get(key) ?? '').trim();
	return raw ? int(form, key) : null;
}

export function bool(form: FormData, key: string): boolean {
	return form.get(key) === 'on' || form.get(key) === 'true' || form.get(key) === '1';
}

export function httpsUrl(value: string | null): string | null {
	if (value && !/^https:\/\//i.test(value))
		throw new UserError('Avatar URL must start with https://');
	return value;
}

export function parsePoints(raw: string): number[] {
	const values = raw
		.split(/[,\s]+/)
		.filter(Boolean)
		.map(Number);
	if (!values.length || values.some((v) => !Number.isInteger(v) || v < 0)) {
		throw new UserError('Points per placement must be whole numbers, e.g. "6, 4, 3, 2, 1, 0".');
	}
	return values;
}

/**
 * Apply a database change, then re-export the JSON and validate it.
 * User-facing problems become a 400 with a message instead of a crash.
 */
export async function mutate(message: string, change: () => unknown) {
	try {
		await change();
	} catch (error) {
		if (error instanceof UserError) return fail(400, { error: error.message });
		if (error instanceof Error && /constraint|FOREIGN KEY|UNIQUE|CHECK/i.test(error.message)) {
			return fail(400, { error: `The database rejected the change: ${error.message}` });
		}
		throw error;
	}
	const validation = await afterChange();
	return { message, errors: validation.errors, warnings: validation.warnings };
}
