/**
 * Fusión local de tarjetas repetidas (vasak-contacts#5): lo que se une, lo que
 * no se pierde y lo que se recuerda.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { emptyContact } from '@/tools/address-book';
import { applyLocalMerges, loadMerges, mergeContacts, saveMerges } from '@/tools/merge';

afterEach(() => globalThis.localStorage?.clear());

const card = (url: string, over = {}) => emptyContact({ url, uid: url, name: url, ...over });

describe('mergeContacts', () => {
	test('la primera tarjeta manda en nombre, organización y url', () => {
		const merged = mergeContacts([
			card('a', { name: 'Ana', organization: 'Vasak' }),
			card('b', { name: 'Anita', organization: 'Otra' }),
		]);
		expect(merged.name).toBe('Ana');
		expect(merged.organization).toBe('Vasak');
		expect(merged.url).toBe('a');
	});

	test('los correos se juntan sin repetir, sin importar mayúsculas ni espacios', () => {
		const merged = mergeContacts([
			card('a', { emails: [{ label: '', value: 'ana@x.com' }] }),
			card('b', {
				emails: [
					{ label: '', value: ' ANA@x.com ' },
					{ label: '', value: 'otro@x.com' },
				],
			}),
		]);
		expect(merged.emails.map((e) => e.value)).toEqual(['ana@x.com', 'otro@x.com']);
	});

	test('los teléfonos se comparan por dígitos exactos: las dos formas quedan', () => {
		const merged = mergeContacts([
			card('a', {
				phones: [
					{ label: '', value: '+54 11 5555 1234' },
					{ label: '', value: '11 5555 1234' },
				],
			}),
			card('b', { phones: [{ label: '', value: '(+54) 11-5555-1234' }] }),
		]);
		expect(merged.phones.map((p) => p.value)).toEqual(['+54 11 5555 1234', '11 5555 1234']);
	});

	test('las notas no vacías se concatenan sin repetir las idénticas', () => {
		const merged = mergeContacts([
			card('a', { notes: 'uno' }),
			card('b', { notes: '' }),
			card('c', { notes: 'dos' }),
			card('d', { notes: 'uno' }),
		]);
		expect(merged.notes).toBe('uno\n\ndos');
	});

	test('categorías e idiomas se unen sin repetir', () => {
		const merged = mergeContacts([
			card('a', { categories: ['Amigos', 'Trabajo'], languages: ['es'] }),
			card('b', { categories: ['Trabajo', 'Club'], languages: ['es', 'en'] }),
		]);
		expect(merged.categories).toEqual(['Amigos', 'Trabajo', 'Club']);
		expect(merged.languages).toEqual(['es', 'en']);
	});
});

describe('applyLocalMerges', () => {
	test('reemplaza el grupo por la unida en el lugar del primero y deja a los demás', () => {
		const list = [card('x'), card('a', { name: 'Ana' }), card('y'), card('b', { name: 'Anita' })];
		const { contacts, members } = applyLocalMerges(list, [['a', 'b']]);
		expect(contacts.map((c) => c.url)).toEqual(['x', 'a', 'y']);
		expect(contacts[1].name).toBe('Ana');
		expect(members.get('a')?.map((c) => c.url)).toEqual(['a', 'b']);
	});

	test('un grupo con un solo miembro presente no fusiona', () => {
		const list = [card('a'), card('c')];
		const { contacts, members } = applyLocalMerges(list, [['a', 'borrada']]);
		expect(contacts).toEqual(list);
		expect(members.size).toBe(0);
	});

	test('sin grupos la lista queda igual', () => {
		const list = [card('a'), card('b')];
		expect(applyLocalMerges(list, []).contacts).toEqual(list);
	});
});

describe('loadMerges / saveMerges', () => {
	test('lo guardado se lee igual', () => {
		saveMerges([['a', 'b'], ['c', 'd', 'e']]);
		expect(loadMerges()).toEqual([['a', 'b'], ['c', 'd', 'e']]);
	});

	test('descarta grupos de menos de dos y la basura', () => {
		globalThis.localStorage.setItem(
			'vasak-contacts.local-merges',
			JSON.stringify([['solo'], 'texto', 7, ['a', 5, 'b'], ['c', 'd']])
		);
		expect(loadMerges()).toEqual([['a', 'b'], ['c', 'd']]);
	});

	test('un JSON roto no deja ninguna', () => {
		globalThis.localStorage.setItem('vasak-contacts.local-merges', '{no');
		expect(loadMerges()).toEqual([]);
	});
});
