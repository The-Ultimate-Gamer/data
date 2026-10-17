import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { DatabaseSync, type SQLInputValue } from 'node:sqlite';
import { config } from './config.ts';

let connection: DatabaseSync | undefined;

export function db(): DatabaseSync {
	if (!connection) {
		if (!existsSync(config.db)) {
			throw new Error(
				`Database not found at ${config.db}. Run "python3 scripts/init_db.py && python3 scripts/seed.py" in the data repository.`
			);
		}
		// Bring older databases up to date with schema.sql before first use.
		execFileSync(config.python, [
			path.join(config.repo, 'scripts', 'migrate.py'),
			'--db',
			config.db
		]);
		connection = new DatabaseSync(config.db);
		connection.exec('PRAGMA foreign_keys = ON');
		connection.exec('PRAGMA busy_timeout = 3000');
	}
	return connection;
}

export function all<T>(sql: string, ...params: SQLInputValue[]): T[] {
	return db()
		.prepare(sql)
		.all(...params) as T[];
}

export function get<T>(sql: string, ...params: SQLInputValue[]): T | undefined {
	return db()
		.prepare(sql)
		.get(...params) as T | undefined;
}

export function run(sql: string, ...params: SQLInputValue[]) {
	return db()
		.prepare(sql)
		.run(...params);
}

let depth = 0;

/** Run `fn` in a transaction; any thrown error rolls everything back. Nested calls join the outer one. */
export function transaction<T>(fn: () => T): T {
	if (depth > 0) return fn();
	const conn = db();
	conn.exec('BEGIN');
	depth++;
	try {
		const result = fn();
		conn.exec('COMMIT');
		return result;
	} catch (error) {
		conn.exec('ROLLBACK');
		throw error;
	} finally {
		depth--;
	}
}
