<script lang="ts">
	import '#lib/styles/admin.css';
	import favicon from '#lib/assets/favicon.svg';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import type { LayoutProps } from './$types';

	let { children, data }: LayoutProps = $props();

	const links = [
		{ href: resolve('/'), label: 'Dashboard' },
		{ href: resolve('/tournaments'), label: 'Tournaments' },
		{ href: resolve('/players'), label: 'Players' },
		{ href: resolve('/games'), label: 'Games' },
		{ href: resolve('/publish'), label: 'Publish' }
	];

	function active(href: string) {
		const path = page.url.pathname;
		return href === resolve('/') ? path === href : path === href || path.startsWith(`${href}/`);
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Admin · The Ultimate Gamer</title>
</svelte:head>

<div class="shell">
	<aside>
		<a class="brand" href={resolve('/')}>
			<span class="mark">UG</span>
			<span>Admin</span>
		</a>
		<nav aria-label="Admin">
			{#each links as link (link.href)}
				<a href={link.href} aria-current={active(link.href) ? 'page' : undefined}>
					{link.label}
					{#if link.label === 'Publish' && data.pendingChanges > 0}
						<span class="count" title="Unpublished changes">{data.pendingChanges}</span>
					{/if}
				</a>
			{/each}
		</nav>
		<p class="db" title={data.dbPath}>
			{#if data.isAlternateDb}<span class="badge danger">Not the real database</span>{/if}
			<span class="subtle">{data.dbPath.split('/').slice(-2).join('/')}</span>
		</p>
	</aside>

	<main>
		{@render children()}
	</main>
</div>

<style>
	.shell {
		display: grid;
		grid-template-columns: 13rem 1fr;
		min-height: 100vh;
	}

	aside {
		position: sticky;
		top: 0;
		height: 100vh;
		display: flex;
		flex-direction: column;
		gap: var(--sp-5);
		padding: var(--sp-5) var(--sp-4);
		border-right: var(--border);
		background: var(--c-bg-raised);
	}

	.brand {
		display: flex;
		align-items: center;
		gap: var(--sp-2);
		color: var(--c-text);
		font-family: var(--font-display);
		font-weight: 800;
		text-transform: uppercase;
		letter-spacing: var(--tracking-wide);
	}

	.mark {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		background: var(--c-gold);
		color: var(--c-on-gold);
		clip-path: polygon(18% 0, 100% 0, 100% 82%, 82% 100%, 0 100%, 0 18%);
		font-size: 0.85rem;
	}

	nav {
		display: grid;
		gap: 0.2rem;
	}

	nav a {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 0.75rem;
		border-radius: var(--radius-md);
		color: var(--c-text-muted);
		font-weight: 600;
	}

	nav a:hover {
		color: var(--c-text);
		background: var(--c-surface);
	}

	nav a[aria-current='page'] {
		color: var(--c-text);
		background: var(--c-surface-2);
		box-shadow: inset 3px 0 0 var(--c-gold);
	}

	.count {
		min-width: 1.4rem;
		padding: 0 0.35rem;
		border-radius: 999px;
		background: var(--c-gold);
		color: var(--c-on-gold);
		font-size: var(--fs-xs);
		text-align: center;
	}

	.db {
		margin-top: auto;
		display: grid;
		gap: var(--sp-1);
		overflow-wrap: anywhere;
	}

	main {
		padding: var(--sp-6);
		min-width: 0;
		display: grid;
		gap: var(--sp-5);
		align-content: start;
		max-width: 90rem;
	}

	@media (max-width: 760px) {
		.shell {
			grid-template-columns: 1fr;
		}

		aside {
			position: static;
			height: auto;
			flex-direction: row;
			flex-wrap: wrap;
			align-items: center;
		}

		nav {
			display: flex;
			flex-wrap: wrap;
		}

		.db {
			margin: 0;
		}

		main {
			padding: var(--sp-4);
		}
	}
</style>
