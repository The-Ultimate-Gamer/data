import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { config } from './config.ts';

const exec = promisify(execFile);
const PATHS = ['database', 'data'];

async function git(...args: string[]) {
	const { stdout } = await exec('git', args, { cwd: config.repo });
	return stdout;
}

export interface Change {
	status: 'new' | 'changed' | 'removed';
	file: string;
	/** Human-readable description of what this file is on the website. */
	label: string;
}

export interface GitStatus {
	branch: string;
	changes: Change[];
	/** The database file itself has uncommitted changes. */
	dbChanged: boolean;
	/** Commits made locally but not pushed yet. */
	ahead: number;
	head: string;
	actionsUrl: string | null;
	error?: string;
}

function describe(file: string): string | null {
	const rel = file.replace(/^data\//, '');
	if (file.startsWith('database/')) return null;
	const fixed: Record<string, string> = {
		'site.json': 'Site settings',
		'leaderboard.json': 'Homepage standings',
		'categories.json': 'Categories',
		'awards.json': 'Awards page',
		'games/index.json': 'Games page',
		'players/index.json': 'Players page',
		'tournaments/index.json': 'Tournaments page'
	};
	if (fixed[rel]) return fixed[rel];
	const player = rel.match(/^players\/(.+)\.json$/);
	if (player) return `Player profile: ${player[1]}`;
	const tournament = rel.match(/^tournaments\/(.+)\.json$/);
	if (tournament) return `Tournament page: ${tournament[1]}`;
	return rel;
}

function parseStatus(code: string): Change['status'] {
	if (code.includes('?') || code.includes('A')) return 'new';
	if (code.includes('D')) return 'removed';
	return 'changed';
}

async function actionsUrl(): Promise<string | null> {
	try {
		const remote = (await git('remote', 'get-url', 'origin')).trim();
		const m = remote.match(/github\.com[:/](.+?)(?:\.git)?$/);
		return m ? `https://github.com/${m[1]}/actions` : null;
	} catch {
		return null;
	}
}

export async function status(): Promise<GitStatus> {
	try {
		const branch = (await git('rev-parse', '--abbrev-ref', 'HEAD')).trim();
		const head = (await git('rev-parse', 'HEAD')).trim();
		const porcelain = await git('status', '--porcelain', '--untracked-files=all', '--', ...PATHS);
		const changes = porcelain
			.split('\n')
			.filter(Boolean)
			.flatMap((line) => {
				const file = line.slice(3);
				const label = describe(file);
				return label ? [{ status: parseStatus(line.slice(0, 2)), file, label }] : [];
			});
		let ahead = 0;
		try {
			ahead = Number((await git('rev-list', '--count', '@{upstream}..HEAD')).trim());
		} catch {
			// No upstream configured.
		}
		return {
			branch,
			changes,
			dbChanged: porcelain.includes('database/'),
			ahead,
			head,
			actionsUrl: await actionsUrl()
		};
	} catch (error) {
		return {
			branch: '?',
			changes: [],
			dbChanged: false,
			ahead: 0,
			head: '',
			actionsUrl: null,
			error: (error as Error).message
		};
	}
}

/** True when the database or JSON differ from what is on GitHub. */
export function isUnpublished(s: GitStatus): boolean {
	return s.changes.length > 0 || s.dbChanged || s.ahead > 0;
}

/** Commit only the database and generated JSON. Returns false when there was nothing to commit. */
export async function commit(message: string): Promise<boolean> {
	await git('add', '--all', '--', ...PATHS);
	const staged = await git('diff', '--cached', '--name-only', '--', ...PATHS);
	if (!staged.trim()) return false;
	await git('commit', '-m', message, '--', ...PATHS);
	return true;
}

export async function push(): Promise<void> {
	await exec('git', ['push'], { cwd: config.repo });
}

export interface Deployment {
	state: 'unknown' | 'running' | 'success' | 'failed';
	url: string | null;
	/** Whether the latest workflow run is for the current commit. */
	current: boolean;
}

/** Latest GitHub Actions run for the publish workflow (requires the GitHub CLI; otherwise 'unknown'). */
export async function deployment(head: string): Promise<Deployment> {
	try {
		const { stdout } = await exec(
			'gh',
			[
				'run',
				'list',
				'--workflow',
				'publish.yml',
				'--limit',
				'1',
				'--json',
				'status,conclusion,url,headSha'
			],
			{ cwd: config.repo, timeout: 8000 }
		);
		const [run] = JSON.parse(stdout) as {
			status: string;
			conclusion: string;
			url: string;
			headSha: string;
		}[];
		if (!run) return { state: 'unknown', url: null, current: false };
		const state =
			run.status !== 'completed' ? 'running' : run.conclusion === 'success' ? 'success' : 'failed';
		return { state, url: run.url, current: run.headSha === head };
	} catch {
		return { state: 'unknown', url: null, current: false };
	}
}
