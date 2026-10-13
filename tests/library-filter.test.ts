/**
 * Elegir qué libretas se ven y filtrar por categoría (vasak-contacts#4).
 *
 * La lógica aparte de la ventana: qué cuenta esconde, qué categoría junta y
 * qué se recuerda entre sesiones. Son las reglas que se equivocan en silencio.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { emptyContact } from '@/tools/address-book';
import {
	bookKey,
	collectAddressBooks,
	collectCategories,
	contactBookKey,
	filterByBooks,
	filterByCategories,
	loadHiddenBooks,
	saveHiddenBooks,
} from '@/tools/library-filter';

function contact(account: string, book: string, categories: string[] = []) {
	return emptyContact({
		account_id: account,
		address_book_id: book,
		categories,
		url: `${account}:${book}:${categories.join(',')}`,
	});
}

describe('la identidad de una libreta', () => {
	test('es la cuenta más el id, con un separador que no aparece en un id', () => {
		// Pegar cuenta e id sin separador hace que `a`+`b1` y `a1`+`b` sean la
		// misma clave, y ocultar una ocultaría la otra.
		expect(bookKey('a', 'b1')).not.toBe(bookKey('a1', 'b'));
	});

	test('dos cuentas con una libreta del mismo id no comparten clave', () => {
		// Es el caso de la «Contactos» de cada cuenta: la misma id en dos
		// cuentas distintas.
		expect(bookKey('ana', 'personal')).not.toBe(bookKey('bruno', 'personal'));
	});

	test('la de un contacto sale de su cuenta y su libreta', () => {
		expect(contactBookKey(contact('ana', 'personal'))).toBe(bookKey('ana', 'personal'));
	});
});

describe('juntar las libretas de las cuentas', () => {
	test('en el orden en que llegaron y sin repetir', () => {
		const books = collectAddressBooks([
			{
				accountId: 'ana',
				accountName: 'ana@nube',
				books: [
					{ url: 'personal', nombre: 'Personal' },
					{ url: 'trabajo', nombre: 'Trabajo' },
				],
			},
			{
				accountId: 'bruno',
				accountName: 'bruno@otra',
				books: [{ url: 'personal', nombre: 'Personal' }],
			},
		]);

		expect(books.map((b) => b.name)).toEqual(['Personal', 'Trabajo', 'Personal']);
		// Dos libretas con el mismo id pero de distinta cuenta son dos entradas.
		expect(books.map((b) => b.key)).toEqual([
			bookKey('ana', 'personal'),
			bookKey('ana', 'trabajo'),
			bookKey('bruno', 'personal'),
		]);
		expect(books[2]?.accountName).toBe('bruno@otra');
	});

	test('la misma libreta repetida en una cuenta se lista una vez', () => {
		const books = collectAddressBooks([
			{
				accountId: 'ana',
				accountName: 'ana@nube',
				books: [
					{ url: 'personal', nombre: 'Personal' },
					{ url: 'personal', nombre: 'Personal' },
				],
			},
		]);
		expect(books).toHaveLength(1);
	});
});

describe('qué libretas se ven', () => {
	const ana = contact('ana', 'personal');
	const trabajo = contact('ana', 'trabajo');
	const bruno = contact('bruno', 'personal');

	test('sin ninguna oculta, están todos', () => {
		expect(filterByBooks([ana, trabajo, bruno], new Set())).toEqual([ana, trabajo, bruno]);
	});

	test('una libreta oculta saca sólo a los suyos, y no a los de otra cuenta con el mismo id', () => {
		const hidden = new Set([bookKey('ana', 'trabajo')]);
		expect(filterByBooks([ana, trabajo, bruno], hidden)).toEqual([ana, bruno]);

		// La «personal» de ana oculta no esconde la «personal» de bruno.
		const soloAna = new Set([bookKey('ana', 'personal')]);
		expect(filterByBooks([ana, bruno], soloAna)).toEqual([bruno]);
	});

	test('lo que llegó sin libreta se muestra: no listada no es escondida', () => {
		const sinLibreta = contact('ana', '');
		expect(filterByBooks([sinLibreta], new Set([bookKey('ana', 'trabajo')]))).toEqual([
			sinLibreta,
		]);
	});
});

describe('las categorías', () => {
	test('se juntan sin repetir, sin distinguir mayúsculas ni acentos, y ordenadas', () => {
		const contacts = [
			contact('ana', 'personal', ['Trabajo', 'Amigos']),
			contact('ana', 'personal', ['trabajo', 'Ñoños']),
			contact('ana', 'personal', ['Álvarez']),
		];
		// «Trabajo» y «trabajo» son una; el orden es el del idioma (la Á va con
		// la A, la Ñ después de la N).
		expect(collectCategories(contacts, 'es')).toEqual(['Álvarez', 'Amigos', 'Ñoños', 'Trabajo']);
	});

	test('las vacías no cuentan', () => {
		expect(collectCategories([contact('ana', 'personal', ['  ', ''])], 'es')).toEqual([]);
	});

	test('sin ninguna elegida, están todos', () => {
		const contacts = [contact('ana', 'personal', ['Trabajo']), contact('ana', 'personal', [])];
		expect(filterByCategories(contacts, new Set())).toEqual(contacts);
	});

	test('elegir varias muestra a quien esté en alguna', () => {
		const trabajo = contact('ana', 'personal', ['Trabajo']);
		const amigos = contact('ana', 'personal', ['Amigos']);
		const ninguna = contact('ana', 'personal', []);
		const ambas = contact('ana', 'personal', ['Trabajo', 'Amigos']);

		const selected = new Set(['Trabajo', 'Amigos']);
		expect(filterByCategories([trabajo, amigos, ninguna, ambas], selected)).toEqual([
			trabajo,
			amigos,
			ambas,
		]);
	});
});

describe('recordar qué libretas se ocultaron', () => {
	beforeEach(() => {
		globalThis.localStorage?.clear();
	});
	afterEach(() => {
		globalThis.localStorage?.clear();
	});

	test('se guarda y se vuelve a leer', () => {
		const hidden = new Set([bookKey('ana', 'trabajo'), bookKey('bruno', 'personal')]);
		saveHiddenBooks(hidden);
		expect(loadHiddenBooks()).toEqual(hidden);
	});

	test('sin nada guardado, no hay ninguna oculta', () => {
		expect(loadHiddenBooks()).toEqual(new Set());
	});

	test('un valor ensuciado no oculta nada: ver de más es mejor que esconder sin pedirlo', () => {
		globalThis.localStorage?.setItem('vasak-contacts.hidden-address-books', 'no es json');
		expect(loadHiddenBooks()).toEqual(new Set());

		globalThis.localStorage?.setItem('vasak-contacts.hidden-address-books', '{"a":1}');
		expect(loadHiddenBooks()).toEqual(new Set());

		// Un arreglo con basura adentro deja sólo las cadenas.
		globalThis.localStorage?.setItem(
			'vasak-contacts.hidden-address-books',
			JSON.stringify(['ana\u0000trabajo', 3, null])
		);
		expect(loadHiddenBooks()).toEqual(new Set(['ana\u0000trabajo']));
	});
});
