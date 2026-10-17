import { execFile } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { config } from './config.ts';

const exec = promisify(execFile);

export interface ScriptResult {
	ok: boolean;
	output: string;
}

export interface Validation extends ScriptResult {
	errors: string[];
	warnings: string[];
}

async function python(script: string): Promise<ScriptResult> {
	const args = [path.join(config.repo, 'scripts', script), '--db', config.db, '--out', config.out];
	try {
		const { stdout, stderr } = await exec(config.python, args, { cwd: config.repo });
		return { ok: true, output: `${stdout}${stderr}`.trim() };
	} catch (error) {
		const e = error as { stdout?: string; stderr?: string; message: string };
		return { ok: false, output: `${e.stdout ?? ''}${e.stderr ?? ''}`.trim() || e.message };
	}
}

export async function exportJson(): Promise<ScriptResult> {
	return python('export_json.py');
}

export async function validate(): Promise<Validation> {
	const result = await python('validate.py');
	const lines = result.output.split('\n');
	return {
		...result,
		errors: lines.filter((l) => l.startsWith('error: ')).map((l) => l.slice(7)),
		warnings: lines.filter((l) => l.startsWith('warning: ')).map((l) => l.slice(9))
	};
}

/** Re-export the JSON after a change and report validation, so the UI always reflects derived results. */
export async function afterChange(): Promise<Validation> {
	const exported = await exportJson();
	if (!exported.ok) return { ...exported, errors: [exported.output], warnings: [] };
	return validate();
}

/** Read a generated JSON document's `data` (e.g. 'tournaments/lan-2026.json'), or null. */
export async function readExported<T>(rel: string): Promise<T | null> {
	const file = path.resolve(config.out, rel);
	if (!file.startsWith(config.out + path.sep)) return null;
	try {
		return (JSON.parse(await readFile(file, 'utf-8')) as { data: T }).data;
	} catch {
		return null;
	}
}
