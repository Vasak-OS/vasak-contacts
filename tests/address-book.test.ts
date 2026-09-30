import { describe, expect, test } from 'bun:test';
import {
	type Contact,
	emptyContact,
	initialOf,
	searchContacts,
	sortContacts,
	withoutAccents,
} from '../src/tools/address-book';

function contact(name: string, extra: Partial<Contact> = {}): Contact {
	return emptyContact({ uid: name, name, sort_key: name, ...extra });
}

describe('sortContacts', () => {
	test('los apellidos con acento van donde corresponde', () => {
		// Comparar por código de carácter manda «Álvarez» después de «Zaparte»,
		// porque la Á está por encima de cualquier letra ASCII: en español eso
		// desordena media agenda.
		const lista = [
			contact('Zaparte, Juan'),
			contact('Álvarez, Ana'),
			contact('Núñez, Bruno'),
			contact('Nogueira, Carla'),
		];

		expect(sortContacts(lista, 'es').map((c) => c.sort_key)).toEqual([
			'Álvarez, Ana',
			'Nogueira, Carla',
			'Núñez, Bruno',
			'Zaparte, Juan',
		]);
	});

	test('las mayúsculas no separan a los Pérez de los pérez', () => {
		const lista = [contact('pérez, Bruno'), contact('Peralta, Ana')];
		expect(sortContacts(lista, 'es').map((c) => c.sort_key)).toEqual([
			'Peralta, Ana',
			'pérez, Bruno',
		]);
	});

	test('no muta la lista que le dieron', () => {
		// Ordenar el arreglo original mutaría lo que vino del programa, y en Vue
		// eso dispara cada dependencia que lo esté mirando.
		const lista = [contact('Zaparte'), contact('Álvarez')];
		const antes = lista.map((c) => c.sort_key);

		sortContacts(lista, 'es');
		expect(lista.map((c) => c.sort_key)).toEqual(antes);
	});

	test('una lista vacía no rompe nada', () => {
		expect(sortContacts([], 'es')).toEqual([]);
	});
});

describe('withoutAccents', () => {
	test('saca las tildes y baja las mayúsculas', () => {
		expect(withoutAccents('Pérez')).toBe('perez');
		expect(withoutAccents('Núñez')).toBe('nunez');
		expect(withoutAccents('ÁÉÍÓÚ')).toBe('aeiou');
	});

	test('lo que no tiene acentos queda igual', () => {
		expect(withoutAccents('Ana')).toBe('ana');
		expect(withoutAccents('')).toBe('');
	});
});

describe('searchContacts', () => {
	const agenda = [
		contact('Ana Pérez', {
			sort_key: 'Pérez, Ana',
			organization: 'Vasak Group',
			emails: [{ label: 'work', value: 'ana@vasak.net.ar' }],
			phones: [{ label: 'cell', value: '+54 11 5555-1234' }],
		}),
		contact('Juan Gómez', {
			sort_key: 'Gómez, Juan',
			organization: 'Panadería del barrio',
			emails: [{ label: 'home', value: 'juan@ejemplo.com' }],
		}),
	];

	test('buscar sin acentos encuentra igual', () => {
		// Nadie escribe los acentos en un buscador, ni siquiera quien los
		// escribe bien en todo lo demás.
		expect(searchContacts(agenda, 'perez')).toHaveLength(1);
		expect(searchContacts(agenda, 'Perez')[0].name).toBe('Ana Pérez');
		expect(searchContacts(agenda, 'gomez')[0].name).toBe('Juan Gómez');
	});

	test('busca también por organización y por correo', () => {
		// La gente busca por lo que recuerda, y a veces lo que recuerda es «el
		// que trabaja en Vasak».
		expect(searchContacts(agenda, 'vasak')[0].name).toBe('Ana Pérez');
		expect(searchContacts(agenda, 'panaderia')[0].name).toBe('Juan Gómez');
		expect(searchContacts(agenda, 'juan@ejemplo')[0].name).toBe('Juan Gómez');
	});

	test('un teléfono se encuentra sin escribir los separadores', () => {
		// Nadie busca «11-5555» poniendo el guión en el mismo lugar.
		expect(searchContacts(agenda, '5555 1234')).toHaveLength(1);
		expect(searchContacts(agenda, '55551234')).toHaveLength(1);
		expect(searchContacts(agenda, '999999')).toHaveLength(0);
	});

	test('cada palabra puede aparecer en una parte distinta', () => {
		// «ana vasak» encuentra a Ana que trabaja en Vasak, que es lo que
		// quiere decir quien lo escribe.
		expect(searchContacts(agenda, 'ana vasak')).toHaveLength(1);
		// Y dos palabras que no están en el mismo contacto no encuentran nada,
		// aunque cada una por su lado encuentre a alguien.
		expect(searchContacts(agenda, 'perez panaderia')).toHaveLength(0);
	});

	test('busca por partes de una palabra, a propósito', () => {
		// Escribir «ana» encuentra a Susana, que es lo que se espera de un
		// buscador de nombres — y también a la panadería, porque «ana» está
		// adentro de «panadería». Ese ruido es el precio de que buscar por la
		// mitad de un nombre funcione, y es el trato que hace cualquier agenda.
		const conSusana = [...agenda, contact('Susana Ruiz', { sort_key: 'Ruiz, Susana' })];
		const encontrados = searchContacts(conSusana, 'ana').map((c) => c.name);

		expect(encontrados).toContain('Susana Ruiz');
		expect(encontrados).toContain('Ana Pérez');
	});

	test('busca también por el apodo y por el cargo', () => {
		// Mucha gente busca a alguien por cómo le dice, no por cómo se llama, y
		// en una libreta de trabajo por lo que hace.
		const conApodo = [
			...agenda,
			contact('Joaquín Decima', {
				sort_key: 'Decima, Joaquín',
				nickname: 'Pato',
				title: 'Jefe de sistemas',
			}),
		];
		expect(searchContacts(conApodo, 'pato')[0].name).toBe('Joaquín Decima');
		expect(searchContacts(conApodo, 'sistemas')[0].name).toBe('Joaquín Decima');
	});

	test('sin consulta se devuelve todo', () => {
		expect(searchContacts(agenda, '')).toHaveLength(2);
		expect(searchContacts(agenda, '   ')).toHaveLength(2);
	});

	test('lo que no está no aparece', () => {
		expect(searchContacts(agenda, 'nadie')).toHaveLength(0);
	});
});

describe('initialOf', () => {
	test('agrupa sin mirar el acento', () => {
		// Álvarez y Alvarez en la misma letra, no en dos grupos separados por
		// todo el alfabeto.
		expect(initialOf(contact('x', { sort_key: 'Álvarez, Ana' }))).toBe('A');
		expect(initialOf(contact('x', { sort_key: 'Alvarez, Ana' }))).toBe('A');
		expect(initialOf(contact('x', { sort_key: 'ñandú' }))).toBe('N');
	});

	test('lo que no empieza con una letra va a su propio grupo', () => {
		// Mezclarlo con la A es peor que darle el suyo.
		expect(initialOf(contact('x', { sort_key: '3 Amigos' }))).toBe('#');
		expect(initialOf(contact('x', { sort_key: '' }))).toBe('#');
		expect(initialOf(contact('x', { sort_key: '   ' }))).toBe('#');
		expect(initialOf(contact('x', { sort_key: '日本' }))).toBe('#');
	});
});

describe('searchContacts, con separadores raros', () => {
	test('un teléfono se encuentra escriba como escriba los separadores', () => {
		// Enumerar el guión, el punto y el paréntesis dejaba afuera «11/5555»,
		// y la lista de lo que la gente usa para separar no es corta.
		const agenda = [
			contact('Ana', {
				phones: [{ label: 'cell', value: '11/5555-1234' }],
			}),
		];

		expect(searchContacts(agenda, '1155551234')).toHaveLength(1);
		expect(searchContacts(agenda, '5555 1234')).toHaveLength(1);
		expect(searchContacts(agenda, '(11) 5555.1234')).toHaveLength(1);
	});
});

describe('searchContacts, el caso que rompió todo una vez', () => {
	test('una palabra sin dígitos no coincide con cualquier cosa', () => {
		// Comparar «los dígitos de la palabra» contra el contacto parece
		// inofensivo hasta que la palabra no tiene ninguno: queda la cadena
		// vacía, todo texto la contiene, y buscar «nadie» devuelve la agenda
		// entera. Se ve como un buscador que no filtra.
		const agenda = [contact('Ana'), contact('Juan')];
		expect(searchContacts(agenda, 'nadie')).toHaveLength(0);
		expect(searchContacts(agenda, 'zzz')).toHaveLength(0);
	});
});
