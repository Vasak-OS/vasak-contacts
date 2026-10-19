/**
 * Qué campos de un contacto van al código QR (vasak-contacts#10).
 *
 * `tools/qr-card.ts` es la regla que se equivoca en silencio: un teléfono de más
 * no se ve en el código, sólo lo nota quien lo escanea. La vCard la arma el
 * programa (`vcard::export`, probado en Rust); acá se prueba lo que le llega.
 */

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { emptyContact } from '@/tools/address-book';
import {
	buildQrCard,
	defaultSelection,
	isQrCardEmpty,
	QR_MAX_EMAILS,
	QR_MAX_PHONES,
	type QrSelection,
	qrErrorKey,
} from '@/tools/qr-card';

const ANA = emptyContact({
	name: ' Ana Pérez ',
	organization: ' Vasak Group ',
	phones: [
		{ label: 'casa', value: '' },
		{ label: 'celular', value: ' 111 ' },
		{ label: 'trabajo', value: '222' },
		{ label: '', value: '333' },
	],
	emails: [
		{ label: '', value: '  ' },
		{ label: 'trabajo', value: ' ana@x.com ' },
		{ label: 'casa', value: 'ana@casa.com' },
	],
});

const NONE: QrSelection = { name: false, phones: [], emails: [], organization: false };

describe('lo que viene marcado al abrir', () => {
	test('son el nombre, los dos primeros teléfonos que existen, el primer correo y la organización', () => {
		expect(defaultSelection(ANA)).toEqual({
			name: true,
			// El 0 está vacío: no cuenta ni ocupa uno de los dos lugares.
			phones: [1, 2],
			emails: [1],
			organization: true,
		});
	});

	test('el tope es el del programa: dos teléfonos y un correo', () => {
		expect(QR_MAX_PHONES).toBe(2);
		expect(QR_MAX_EMAILS).toBe(1);
	});

	test('de lo que el contacto no tiene no se marca nada', () => {
		expect(defaultSelection(emptyContact({ name: '  ' }))).toEqual(NONE);
	});
});

describe('lo que se le manda al programa', () => {
	test('lleva sólo lo marcado, sin blancos de más', () => {
		expect(buildQrCard(ANA, { name: true, phones: [1], emails: [2], organization: false })).toEqual({
			name: 'Ana Pérez',
			phones: ['111'],
			email: 'ana@casa.com',
			organization: '',
		});
	});

	test('sin nada marcado no lleva nada', () => {
		const card = buildQrCard(ANA, NONE);

		expect(card).toEqual({ name: '', phones: [], email: null, organization: '' });
		expect(isQrCardEmpty(card)).toBe(true);
	});

	test('no pasa del tope aunque la selección lo pida', () => {
		const card = buildQrCard(ANA, { ...NONE, phones: [1, 2, 3], emails: [1, 2] });

		expect(card.phones).toEqual(['111', '222']);
		expect(card.email).toBe('ana@x.com');
	});

	test('ignora posiciones repetidas, inexistentes o de un valor vacío, y respeta el orden de la ficha', () => {
		const card = buildQrCard(ANA, { ...NONE, phones: [2, 2, 9, 0, -1, 1], emails: [7] });

		expect(card.phones).toEqual(['111', '222']);
		expect(card.email).toBeNull();
	});

	test('no hay forma de pedir la foto ni las notas', () => {
		const contact = emptyContact({
			name: 'Ana',
			photo: 'data:image/png;base64,AAAA',
			photo_url: 'https://x/ana.jpg',
			notes: 'una nota privada',
		});

		expect(Object.keys(buildQrCard(contact, defaultSelection(contact))).sort()).toEqual([
			'email',
			'name',
			'organization',
			'phones',
		]);
	});

	test('una tarjeta con un solo dato no está vacía', () => {
		expect(isQrCardEmpty({ name: '', phones: [], email: 'a@x.com', organization: '' })).toBe(false);
		expect(isQrCardEmpty({ name: '', phones: ['1'], email: null, organization: '' })).toBe(false);
		expect(isQrCardEmpty({ name: '', phones: [], email: null, organization: 'X' })).toBe(false);
	});
});

describe('los errores del programa', () => {
	test('las marcas se traducen y lo desconocido cae en el mensaje genérico', () => {
		expect(qrErrorKey('qr-empty')).toBe('qr.empty');
		expect(qrErrorKey('qr-too-long')).toBe('qr.tooLong');
		expect(qrErrorKey('Error: algo del sistema')).toBe('qr.error');
		expect(qrErrorKey('')).toBe('qr.error');
	});
});

describe('los textos del código y de la tarjeta propia', () => {
	const catalogs = ['es', 'en'].map((language) => ({
		language,
		catalog: Bun.YAML.parse(
			readFileSync(join(import.meta.dir, `../src-tauri/locales/${language}.yml`), 'utf8')
		) as Record<string, Record<string, unknown>>,
	}));
	const sources = [
		'components/contacts/QrDialog.vue',
		'components/contacts/OwnCardDialog.vue',
		'components/contacts/ContactDetail.vue',
		'views/AddressBookView.vue',
	].map((file) => readFileSync(join(import.meta.dir, '../src', file), 'utf8'));
	const used = new Set([
		...sources.flatMap((text) => [...text.matchAll(/\bt\('((?:qr|ownCard)\.\w+)'\)/g)].map((m) => m[1])),
		// Las que elige `qrErrorKey` por el error del programa.
		...['qr-empty', 'qr-too-long', 'otro'].map(qrErrorKey),
	]);

	test('hay textos que comprobar', () => {
		expect(used.size).toBeGreaterThan(20);
	});

	for (const { language, catalog } of catalogs) {
		test(`cada clave que usa la ventana está en ${language}.yml y no está vacía`, () => {
			for (const key of used) {
				const [group, name] = (key as string).split('.') as [string, string];
				expect(String(catalog[group]?.[name] ?? ''), `${language}.yml: ${key}`).not.toBe('');
			}
		});
	}
});
