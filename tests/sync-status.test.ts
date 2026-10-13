/**
 * La línea de sincronización que va debajo de cada cuenta.
 *
 * `syncLine` decide tono, texto y si el punto late a partir de lo que dice el
 * almacén (`GetStatus`) y de lo que no se pudo pedir con «Actualizar». Acá se
 * comprueba esa decisión, sin ventana, y que **cada clave** que puede devolver
 * exista en los dos catálogos: las claves se arman con el motivo
 * (`sync.request.${reason}`), así que la prueba de claves literales no las ve y
 * una que falte se mostraría cruda.
 */

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	type AccountSyncStatus,
	type SyncLine,
	type SyncRequestFailure,
	syncedWhen,
	syncLine,
} from '@/tools/sync-status';

/** Hoy a la tarde, en la hora local de quien corre la prueba. */
const NOW = new Date(2026, 9, 13, 18, 30);

function status(overrides: Partial<AccountSyncStatus> = {}): AccountSyncStatus {
	return { account_id: 'a1', state: 'synced', detail: '', last_synced_at: null, ...overrides };
}

function failure(reason: SyncRequestFailure['reason']): SyncRequestFailure {
	return { account_id: 'a1', reason };
}

describe('cuándo fue la última vuelta', () => {
	test('si fue hoy se dice la hora', () => {
		const today = new Date(2026, 9, 13, 14, 5).toISOString();
		expect(syncedWhen(today, NOW, 'es')).toBe('14:05');
	});

	test('si fue otro día se dice el día, no una hora que parecería de hoy', () => {
		const yesterday = new Date(2026, 9, 12, 23, 30).toISOString();
		expect(syncedWhen(yesterday, NOW, 'es')).toBe('12 oct');
	});

	test('el mismo día de otro mes o de otro año tampoco es hoy', () => {
		expect(syncedWhen(new Date(2026, 8, 13, 9, 0).toISOString(), NOW, 'es')).toBe('13 sept');
		expect(syncedWhen(new Date(2025, 9, 13, 9, 0).toISOString(), NOW, 'es')).toBe('13 oct');
	});

	test('una fecha que no se entiende no se muestra', () => {
		expect(syncedWhen('no es una fecha', NOW, 'es')).toBe('');
		expect(syncedWhen('', NOW, 'es')).toBe('');
	});
});

describe('qué línea va debajo de la cuenta', () => {
	test('sin estado, o con el área apagada, no se dice nada', () => {
		expect(syncLine(undefined, undefined, NOW)).toBeNull();
		expect(syncLine(status({ state: 'off' }), undefined, NOW)).toBeNull();
		expect(syncLine(status({ state: 'algo-nuevo' }), undefined, NOW)).toBeNull();
	});

	test('esperando para sincronizar late en tono neutro', () => {
		expect(syncLine(status({ state: 'pending' }), undefined, NOW)).toEqual({
			tone: 'neutral',
			key: 'sync.pending',
			arg: '',
			pulse: true,
		});
	});

	test('sincronizando late con el acento', () => {
		expect(syncLine(status({ state: 'syncing' }), undefined, NOW)).toEqual({
			tone: 'accent',
			key: 'sync.syncing',
			arg: '',
			pulse: true,
		});
	});

	test('al día dice a qué hora fue la última vuelta', () => {
		const at = new Date(2026, 9, 13, 14, 5).toISOString();
		expect(syncLine(status({ state: 'synced', last_synced_at: at }), undefined, NOW, 'es')).toEqual(
			{ tone: 'success', key: 'sync.syncedAt', arg: '14:05', pulse: false }
		);
	});

	test('al día sin hora, o con una que no se entiende, dice sólo que está al día', () => {
		const expected: SyncLine = { tone: 'success', key: 'sync.synced', arg: '', pulse: false };
		expect(syncLine(status({ state: 'synced' }), undefined, NOW)).toEqual(expected);
		expect(
			syncLine(status({ state: 'synced', last_synced_at: 'ayer' }), undefined, NOW)
		).toEqual(expected);
	});

	test('si falló se dice en tono de error y con el motivo del almacén', () => {
		expect(
			syncLine(status({ state: 'failed', detail: 'el servidor rechazó la clave' }), undefined, NOW)
		).toEqual({
			tone: 'error',
			key: 'sync.failed',
			arg: 'el servidor rechazó la clave',
			pulse: false,
		});
	});

	test('si no está disponible se avisa, no se muestra como roto', () => {
		expect(
			syncLine(status({ state: 'unavailable', detail: 'no implementado' }), undefined, NOW)
		).toEqual({ tone: 'warning', key: 'sync.unavailable', arg: 'no implementado', pulse: false });
	});

	test('lo que «Actualizar» no pudo pedir gana sobre el estado del almacén', () => {
		const syncing = status({ state: 'syncing' });
		for (const reason of ['denied', 'unavailable', 'failed'] as const) {
			expect(syncLine(syncing, failure(reason), NOW)).toEqual({
				tone: 'warning',
				key: `sync.request.${reason}`,
				arg: '',
				pulse: false,
			});
		}
	});

	test('«se acaba de pedir» no es un problema: va en tono neutro', () => {
		expect(syncLine(status({ state: 'synced' }), failure('too_soon'), NOW)).toEqual({
			tone: 'neutral',
			key: 'sync.request.too_soon',
			arg: '',
			pulse: false,
		});
	});

	test('un pedido rechazado se dice aunque el almacén todavía no conozca la cuenta', () => {
		expect(syncLine(undefined, failure('denied'), NOW)?.key).toBe('sync.request.denied');
	});
});

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
