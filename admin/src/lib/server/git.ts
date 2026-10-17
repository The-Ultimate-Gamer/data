import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { config } from './config.ts';

const exec = promisify(execFile);
const PATHS = ['database', 'data'];

async function git(...args: string[]) {
	const { stdout } = await exec('git', args, { cwd: config.repo });
	return stdout.trim();
}

export interface GitStatus {
	branch: string;
	changes: { status: string; file: string }[];
	ahead: number;
	error?: string;
}

export async function status(): Promise<GitStatus> {
	try {
		const branch = await git('rev-parse', '--abbrev-ref', 'HEAD');
		const porcelain = await git('status', '--porcelain', '--', ...PATHS);
		const changes = porcelain
			.split('\n')
			.filter(Boolean)
			.map((line) => ({ status: line.slice(0, 2).trim(), file: line.slice(3) }));
		let ahead = 0;
		try {
			ahead = Number(await git('rev-list', '--count', '@{upstream}..HEAD'));
		} catch {
			// No upstream configured.
		}
		return { branch, changes, ahead };
	} catch (error) {
		return { branch: '?', changes: [], ahead: 0, error: (error as Error).message };
	}
}

/** Commit only the database and generated JSON. */
export async function commit(message: string): Promise<string> {
	await git('add', '--', ...PATHS);
	return git('commit', '-m', message, '--', ...PATHS);
}

export async function push(): Promise<string> {
	const { stdout, stderr } = await exec('git', ['push'], { cwd: config.repo });
	return `${stdout}${stderr}`.trim();
}
