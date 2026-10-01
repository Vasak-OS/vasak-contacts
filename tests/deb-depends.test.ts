/**
 * Lo que el `.deb` declara tiene que ser lo que el binario enlaza.
 *
 * Medido con `readelf -d … | grep NEEDED` sobre el binario de release, nunca
 * con `ldd`, que suma las transitivas: `libgdk-3`, `libgdk_pixbuf-2.0`,
 * `libcairo`, `libgobject`/`libglib`/`libgio`, `libdbus-1`,
 * `libwebkit2gtk-4.1`, `libgtk-3`, `libsoup-3.0`, `libjavascriptcoregtk-4.1`,
 * `libgcc_s`, `libm` y `libc`. Ninguna capa (`gtk-layer-shell`): es una ventana
 * normal. La lista ya estaba adaptada; esto la ata para que no vuelva la de
 * diez de la plantilla `vapp`, con las dos generaciones de libsoup a la vez.
 */

import { describe, expect, test } from 'bun:test';

const conf = await Bun.file(new URL('../src-tauri/tauri.conf.json', import.meta.url)).json();
const depends: string[] = conf.bundle.linux.deb.depends;

describe('las dependencias del .deb', () => {
	test('traen lo que enlaza el binario', () => {
		for (const pkg of [
			'libc6',
			'libgcc-s1',
			'libcairo2',
			'libdbus-1-3',
			'libgdk-pixbuf-2.0-0',
			'libglib2.0-0t64',
			'libgtk-3-0t64',
			'libjavascriptcoregtk-4.1-0',
			'libsoup-3.0-0',
			'libwebkit2gtk-4.1-0',
		]) {
			expect(depends, `falta ${pkg}`).toContain(pkg);
		}
	});

	test('y no lo que no enlaza', () => {
		// libsoup 2 es la de WebKitGTK 4.0; Tauri 2 va con la 4.1, que usa la 3.
		// Y no es una superficie de capa: no enlaza gtk-layer-shell.
		for (const pkg of ['libsoup2.4-1', 'libpango-1.0-0', 'libgtk-layer-shell0', 'libssl3']) {
			expect(depends).not.toContain(pkg);
		}
	});

	test('sin repetidos', () => {
		expect(new Set(depends).size).toBe(depends.length);
	});
});
