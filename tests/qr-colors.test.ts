/**
 * El código QR se dibuja oscuro sobre claro en **los dos modos** (vasak-contacts#10).
 *
 * Un QR es un código funcional, no un adorno: las cámaras leen mal el invertido
 * (claro sobre oscuro), y con el esquema oscuro puesto es justo lo que saldría si
 * el código usara los colores que siguen al modo (`text-tx-main`, `bg-ui-bg`,
 * que son `--use-*`). Por eso es la excepción a la regla de que todo color sale
 * del modo actual: usa `--text-main` y `--ui-background`, que son **siempre** los
 * del esquema claro (los del oscuro son `--text-main-dark` y
 * `--ui-background-dark`; el modo sólo cambia a cuál apunta `--use-*`).
 *
 * Esta prueba guarda las tres cosas de las que eso depende:
 *
 * 1. que el diálogo use esos dos tokens y no los que siguen al modo;
 * 2. que nada vuelva a asignar `--text-main` / `--ui-background` según el modo
 *    (ni la hoja de la aplicación ni el config-manager que escribe el esquema);
 * 3. que el esquema claro de arranque sea de verdad oscuro sobre claro, con
 *    contraste de sobra.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { ActionButton } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import QrDialog from '@/components/contacts/QrDialog.vue';
import { emptyContact } from '@/tools/address-book';
import { answerCommand, olvidarTodo } from './dobles';

const ROOT = join(import.meta.dir, '..');
const read = (path: string) => readFileSync(join(ROOT, path), 'utf8');

const MAIN_CSS = read('src/assets/main.css');
const QR_DIALOG = read('src/components/contacts/QrDialog.vue');

/** El cuerpo de la primera regla `selector { … }` de una hoja, sin comentarios. */
function ruleBody(css: string, selector: string): string {
	const clean = css.replace(/\/\*[\s\S]*?\*\//g, '');
	const start = clean.indexOf(`${selector} {`);
	if (start < 0) {
		throw new Error(`no hay una regla ${selector}`);
	}
	const open = clean.indexOf('{', start);
	let depth = 0;
	for (let i = open; i < clean.length; i++) {
		if (clean[i] === '{') depth++;
		if (clean[i] === '}' && --depth === 0) {
			return clean.slice(open + 1, i);
		}
	}
	throw new Error(`la regla ${selector} no cierra`);
}

function declared(body: string, name: string): string | null {
	const found = new RegExp(`(?:^|[;\\s])${name}\\s*:\\s*([^;]+);`).exec(body);
	return found ? (found[1] ?? '').trim() : null;
}

function luminance(hex: string): number {
	const [r, g, b] = [1, 3, 5].map((i) => {
		const c = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
		return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	}) as [number, number, number];
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
	const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
	return (light + 0.05) / (dark + 0.05);
}

describe('el código no se invierte con el modo', () => {
	let wrapper: VueWrapper | null = null;
	afterEach(() => {
		wrapper?.unmount();
		wrapper = null;
		olvidarTodo();
	});

	test('el contenedor del código usa los tokens del esquema claro y ninguno que siga al modo', async () => {
		answerCommand('generate_vcard_qr', () => '<svg viewBox="0 0 8 8"><rect width="8" height="8"/></svg>');
		wrapper = mount(QrDialog, {
			attachTo: document.body.appendChild(document.createElement('div')),
			props: { open: true, contact: emptyContact({ name: 'Ana' }) },
		});
		const generate = wrapper.findAllComponents(ActionButton).find((b) => b.props('label') === 'qr.generate');
		await generate?.trigger('click');
		await flushPromises();

		const code = document.body.querySelector('[data-testid="qr-code"]');
		const classes = (code?.getAttribute('class') ?? '').split(/\s+/);

		expect(classes).toContain('bg-[var(--ui-background)]');
		expect(classes).toContain('text-[var(--text-main)]');
		// Lo que cambia con el modo: las utilidades que salen de `--use-*`.
		expect(classes.filter((c) => /^(bg|text|border|fill|stroke)-(ui|tx)-|--use-|-dark\b|^dark:/.test(c))).toEqual([]);
	});

	test('el diálogo no nombra ninguna variable `--use-*` ni la variante `-dark` para pintar el código', () => {
		const block = QR_DIALOG.slice(QR_DIALOG.indexOf('data-testid="qr-code"') - 400, QR_DIALOG.indexOf('data-testid="qr-code"'));

		expect(block).not.toMatch(/--use-/);
		expect(block).not.toMatch(/-dark/);
		expect(block).not.toMatch(/\b(bg-ui-bg|text-tx-main|bg-ui-surface)\b/);
	});

	test('la hoja de la aplicación no cambia `--text-main` ni `--ui-background` en el modo oscuro', () => {
		const dark = ruleBody(MAIN_CSS, '.dark');

		expect(declared(dark, '--text-main')).toBeNull();
		expect(declared(dark, '--ui-background')).toBeNull();
		// El modo oscuro mueve el puntero `--use-*`, no el token.
		expect(declared(dark, '--use-text-main')).toBe('var(--text-main-dark)');
		expect(declared(dark, '--use-ui-background')).toBe('var(--ui-background-dark)');
	});

	test('ninguna hoja de la aplicación redefine esos tokens según el tema del sistema', () => {
		expect(MAIN_CSS).not.toMatch(/prefers-color-scheme/);
		const light = ruleBody(MAIN_CSS, ':root');
		expect(declared(light, '--use-text-main')).toBe('var(--text-main)');
		expect(declared(light, '--use-ui-background')).toBe('var(--ui-background)');
	});

	test('el config-manager escribe en esos tokens el esquema claro, y el oscuro en los `-dark`', () => {
		const plugin = read('node_modules/@vasakgroup/plugin-config-manager/dist-js/index.js');

		expect(plugin).toMatch(/setProperty\("--text-main",\s*lightScheme\.ui\.text\.main\)/);
		expect(plugin).toMatch(/setProperty\("--ui-background",\s*lightScheme\.ui\.background\)/);
		expect(plugin).toMatch(/setProperty\("--text-main-dark",\s*darkScheme\.ui\.text\.main\)/);
		expect(plugin).toMatch(/setProperty\("--ui-background-dark",\s*darkScheme\.ui\.background\)/);
	});
});

describe('el esquema claro de arranque es oscuro sobre claro', () => {
	const light = ruleBody(MAIN_CSS, ':root');
	const text = declared(light, '--text-main') as string;
	const background = declared(light, '--ui-background') as string;

	test('los dos son colores hexadecimales que se pueden medir', () => {
		expect(text).toMatch(/^#[0-9a-f]{6}$/i);
		expect(background).toMatch(/^#[0-9a-f]{6}$/i);
	});

	test('los módulos son más oscuros que el fondo y el contraste pasa de 7', () => {
		expect(luminance(text)).toBeLessThan(luminance(background));
		expect(contrast(text, background)).toBeGreaterThanOrEqual(7);
	});
});
