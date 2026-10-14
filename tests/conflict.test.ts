/**
 * Leer una vCard a lo justo para comparar dos versiones de un contacto en
 * conflicto.
 *
 * Es la parte que se equivoca **en silencio**: leer mal una tarjeta mostraría la
 * versión de otro, y la persona elegiría a ciegas cuál queda. Por eso se prueba
 * que el nombre salga de `FN` —o se arme con `N` si no hay—, que las secuencias
 * con barra se deshagan, que las líneas plegadas se junten, y que una tarjeta
 * que no se puede leer caiga al crudo formateado en vez de a una ficha vacía.
 */

import { describe, expect, test } from 'bun:test';
import {
	formattedRaw,
	hasReadableFields,
	readableContact,
} from '@/tools/conflict';

describe('leer una vCard para comparar', () => {
	test('saca el nombre, la organización, los correos y los teléfonos', () => {
		const raw =
			'BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana Pérez\r\nORG:Vasak;Soporte\r\n' +
			'EMAIL;TYPE=work:ana@x.com\r\nEMAIL:ana@casa.com\r\nTEL;TYPE=cell:+54 11 5555\r\nEND:VCARD\r\n';
		const read = readableContact(raw);
		expect(read.name).toBe('Ana Pérez');
		// La organización es la primera parte de `ORG`, no «Vasak;Soporte».
		expect(read.organization).toBe('Vasak');
		expect(read.emails).toEqual(['ana@x.com', 'ana@casa.com']);
		expect(read.phones).toEqual(['+54 11 5555']);
	});

	test('sin `FN`, arma el nombre con `N` (nombre apellido)', () => {
		const raw = 'BEGIN:VCARD\r\nVERSION:3.0\r\nN:Pérez;Ana;;;\r\nEND:VCARD\r\n';
		expect(readableContact(raw).name).toBe('Ana Pérez');
	});

	test('`FN` gana sobre `N` cuando están los dos', () => {
		const raw = 'BEGIN:VCARD\r\nN:Pérez;Ana;;;\r\nFN:Ana P.\r\nEND:VCARD\r\n';
		expect(readableContact(raw).name).toBe('Ana P.');
	});

	test('deshace las secuencias con barra del valor', () => {
		const raw = 'BEGIN:VCARD\r\nFN:Pérez\\, Ana\r\nORG:Vasak \\; Cía\r\nEND:VCARD\r\n';
		const read = readableContact(raw);
		expect(read.name).toBe('Pérez, Ana');
	});

	test('junta una línea plegada (continuación con espacio)', () => {
		const raw = 'BEGIN:VCARD\r\nFN:Ana Mar\r\n ía Pérez\r\nEND:VCARD\r\n';
		expect(readableContact(raw).name).toBe('Ana María Pérez');
	});

	test('una tarjeta sin nada legible no inventa campos', () => {
		const raw = 'BEGIN:VCARD\r\nVERSION:3.0\r\nUID:x\r\nEND:VCARD\r\n';
		const read = readableContact(raw);
		expect(read.name).toBe('');
		expect(read.organization).toBe('');
		expect(read.emails).toEqual([]);
		expect(read.phones).toEqual([]);
		expect(hasReadableFields(read)).toBe(false);
	});

	test('con cualquier campo con valor, hay algo que mostrar', () => {
		expect(hasReadableFields(readableContact('BEGIN:VCARD\r\nFN:Ana\r\nEND:VCARD\r\n'))).toBe(true);
		expect(
			hasReadableFields(readableContact('BEGIN:VCARD\r\nEMAIL:a@x.com\r\nEND:VCARD\r\n'))
		).toBe(true);
	});
});

describe('el crudo formateado, de respaldo', () => {
	test('saca el armazón (BEGIN/END/VERSION) y junta lo plegado', () => {
		const raw =
			'BEGIN:VCARD\r\nVERSION:3.0\r\nUID:abc\r\nX-RARO:algo raro\r\nEND:VCARD\r\n';
		const text = formattedRaw(raw);
		expect(text).not.toContain('BEGIN');
		expect(text).not.toContain('END:VCARD');
		expect(text).not.toContain('VERSION');
		expect(text).toContain('UID:abc');
		expect(text).toContain('X-RARO:algo raro');
	});

	test('una vCard vacía da texto vacío, no una línea de armazón', () => {
		expect(formattedRaw('BEGIN:VCARD\r\nEND:VCARD\r\n')).toBe('');
	});
});
