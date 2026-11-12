/**
 * Detección de posibles repetidos (vasak-contacts#5). Sólo propone: la fusión
 * viene aparte.
 */

import { describe, expect, test } from 'bun:test';
import { type Contact, emptyContact } from '@/tools/address-book';
import { duplicateCount, findDuplicates } from '@/tools/duplicates';

function card(
	name: string,
	extra: { uid?: string; emails?: string[]; phones?: string[] } = {}
): Contact {
	return emptyContact({
		name,
		uid: extra.uid ?? '',
		emails: (extra.emails ?? []).map((value) => ({ label: '', value })),
		phones: (extra.phones ?? []).map((value) => ({ label: '', value })),
	});
}

const names = (groups: { contacts: Contact[] }[]) => groups.map((g) => g.contacts.map((c) => c.name));

describe('findDuplicates', () => {
	test('el mismo UID, con y sin urn:uuid:, junta solo aunque no compartan nada más', () => {
		const a = card('Ana', { uid: 'urn:uuid:abc-123' });
		const b = card('Otra Persona', { uid: 'abc-123' });
		expect(names(findDuplicates([a, b]))).toEqual([['Ana', 'Otra Persona']]);
	});

	test('una sola señal débil (correo, teléfono o nombre) no junta', () => {
		expect(findDuplicates([card('Ana', { emails: ['a@x.com'] }), card('Beto', { emails: ['a@x.com'] })])).toEqual([]);
		expect(
			findDuplicates([card('Ana', { phones: ['1155551234'] }), card('Beto', { phones: ['1155551234'] })])
		).toEqual([]);
	});

	test('nunca junta sólo por nombre, ni con acentos distintos', () => {
		expect(findDuplicates([card('Juan Pérez'), card('Juan Perez')])).toEqual([]);
	});

	test('nombre y correo juntan', () => {
		const groups = findDuplicates([
			card('Juan Pérez', { emails: ['j@x.com'] }),
			card('Juan Perez', { emails: ['j@x.com'] }),
		]);
		expect(groups).toHaveLength(1);
	});

	test('nombre y teléfono juntan', () => {
		const groups = findDuplicates([
			card('Ana', { phones: ['1155551234'] }),
			card('ana', { phones: ['1155551234'] }),
		]);
		expect(groups).toHaveLength(1);
	});

	test('el correo se compara sin mayúsculas ni espacios', () => {
		const groups = findDuplicates([
			card('Ana', { emails: ['ana@x.com'] }),
			card('Ana', { emails: ['  ANA@X.com '] }),
		]);
		expect(groups).toHaveLength(1);
	});

	test('el teléfono no se compara por subcadena: un número corto que es el final de otro no junta', () => {
		const groups = findDuplicates([
			card('Ana', { phones: ['551234'] }),
			card('Ana', { phones: ['54 11 5555 1234'] }),
		]);
		expect(groups).toEqual([]);
	});

	test('el teléfono se compara por los últimos dígitos, con o sin prefijo y separadores', () => {
		const groups = findDuplicates([
			card('Ana', { phones: ['+54 11 5555-1234'] }),
			card('Ana', { phones: ['11 5555 1234'] }),
		]);
		expect(groups).toHaveLength(1);
	});

	test('el grupo es transitivo, deja afuera al suelto y conserva el orden de entrada', () => {
		const a = card('Ana', { emails: ['a@x.com'] });
		const solo = card('Zeta', { emails: ['z@x.com'] });
		// c se une a b por correo+teléfono, aunque con a no comparta nada.
		const c = card('Cora', { emails: ['m@x.com'], phones: ['1155559999'] });
		const b = card('Ana', { emails: ['a@x.com', 'm@x.com'], phones: ['1155559999'] });
		const groups = findDuplicates([a, solo, c, b]);
		expect(groups).toHaveLength(1);
		expect(groups[0]?.contacts).toEqual([a, c, b]);
	});

	test('no devuelve grupos de uno', () => {
		expect(findDuplicates([card('Ana'), card('Beto')])).toEqual([]);
		expect(findDuplicates([])).toEqual([]);
	});
});

describe('duplicateCount', () => {
	test('cuenta las tarjetas que caen en algún grupo', () => {
		expect(duplicateCount([])).toBe(0);
		const g = (n: number) => ({ contacts: Array.from({ length: n }, () => card('x')) });
		expect(duplicateCount([g(2), g(3)])).toBe(5);
	});
});
