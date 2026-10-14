/**
 * Los textos de la línea de sincronización que va debajo de cada cuenta.
 *
 * Qué línea corresponde a cada estado lo decide `syncLine`, que ahora es del
 * plugin `@vasakgroup/plugin-vsk-store` y se prueba allá. Los catálogos, en
 * cambio, son de esta aplicación: acá se comprueba que **cada clave** que la
 * línea puede devolver exista en los dos. Las claves se arman con el motivo
 * (`sync.request.${reason}`), así que la prueba de claves literales no las ve y
 * una que falte se mostraría cruda.
 */

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	type AccountSyncStatus,
	type SyncRequestFailure,
	syncLine,
} from '@vasakgroup/plugin-vsk-store';

/** Hoy a la tarde, en la hora local de quien corre la prueba. */
const NOW = new Date(2026, 9, 13, 18, 30);

function status(overrides: Partial<AccountSyncStatus> = {}): AccountSyncStatus {
	return { account_id: 'a1', state: 'synced', detail: '', last_synced_at: null, ...overrides };
}

function failure(reason: SyncRequestFailure['reason']): SyncRequestFailure {
	return { account_id: 'a1', reason };
}

/**
 * Las claves hoja de un `.yml` indentado de a dos espacios, como
 * `sync.request.denied`, con su texto. Que los dos idiomas tengan las mismas
 * claves y los mismos marcadores lo prueba `src-tauri/tests/locales.rs`.
 */
function keysOf(yaml: string): Map<string, string> {
	const keys = new Map<string, string>();
	const path: string[] = [];
	for (const line of yaml.split('\n')) {
		if (!line.trim() || line.trimStart().startsWith('#')) continue;
		const level = (line.length - line.trimStart().length) / 2;
		const match = line.trim().match(/^["']?([A-Za-z0-9_.-]+)["']?:(.*)$/);
		if (!match) continue;
		path.length = level;
		path[level] = match[1];
		if (match[2].trim()) keys.set(path.slice(0, level + 1).join('.'), match[2].trim());
	}
	return keys;
}

describe('los textos de la línea', () => {
	const root = fileURLToPath(new URL('..', import.meta.url));
	const catalogs = {
		es: keysOf(readFileSync(join(root, 'src-tauri/locales/es.yml'), 'utf8')),
		en: keysOf(readFileSync(join(root, 'src-tauri/locales/en.yml'), 'utf8')),
	};

	/** Cada clave que `syncLine` puede devolver. */
	const used = new Set<string>();
	const at = new Date(2026, 9, 13, 14, 5).toISOString();
	for (const state of ['pending', 'syncing', 'synced', 'failed', 'unavailable']) {
		for (const last of [null, at]) {
			const line = syncLine(status({ state, last_synced_at: last }), undefined, NOW);
			if (line) used.add(line.key);
		}
	}
	for (const reason of ['denied', 'too_soon', 'unavailable', 'failed'] as const) {
		const line = syncLine(undefined, failure(reason), NOW);
		if (line) used.add(line.key);
	}

	test('se recorren todas las claves que puede devolver la línea', () => {
		expect(used.size).toBe(10);
	});

	for (const [language, keys] of Object.entries(catalogs)) {
		test(`cada clave de la línea está en ${language}.yml`, () => {
			expect([...used].filter((key) => !keys.has(key)).sort()).toEqual([]);
		});
	}

	test('el texto con la hora o el motivo tiene dónde ponerlos', () => {
		for (const [language, keys] of Object.entries(catalogs)) {
			for (const key of ['sync.syncedAt', 'sync.failed', 'sync.unavailable']) {
				expect(keys.get(key) ?? '', `${language}.yml: ${key}`).toContain('{0}');
			}
		}
	});
});
