import { describe, expect, test } from 'bun:test';
import { buscados, type Contacto, inicialDe, ordenados, sinAcentos } from '../src/tools/agenda';

function contacto(nombre: string, extra: Partial<Contacto> = {}): Contacto {
	return {
		uid: nombre,
		nombre,
		orden: nombre,
		correos: [],
		telefonos: [],
		organizacion: '',
		notas: '',
		url: '',
		...extra,
	};
}

describe('ordenados', () => {
	test('los apellidos con acento van donde corresponde', () => {
		// Comparar por código de carácter manda «Álvarez» después de «Zaparte»,
		// porque la Á está por encima de cualquier letra ASCII: en español eso
		// desordena media agenda.
		const lista = [
			contacto('Zaparte, Juan'),
			contacto('Álvarez, Ana'),
			contacto('Núñez, Bruno'),
			contacto('Nogueira, Carla'),
		];

		expect(ordenados(lista, 'es').map((c) => c.orden)).toEqual([
			'Álvarez, Ana',
			'Nogueira, Carla',
			'Núñez, Bruno',
			'Zaparte, Juan',
		]);
	});

	test('las mayúsculas no separan a los Pérez de los pérez', () => {
		const lista = [contacto('pérez, Bruno'), contacto('Peralta, Ana')];
		expect(ordenados(lista, 'es').map((c) => c.orden)).toEqual([
			'Peralta, Ana',
			'pérez, Bruno',
		]);
	});

	test('no muta la lista que le dieron', () => {
		// Ordenar el arreglo original mutaría lo que vino del programa, y en Vue
		// eso dispara cada dependencia que lo esté mirando.
		const lista = [contacto('Zaparte'), contacto('Álvarez')];
		const antes = lista.map((c) => c.orden);

		ordenados(lista, 'es');
		expect(lista.map((c) => c.orden)).toEqual(antes);
	});

	test('una lista vacía no rompe nada', () => {
		expect(ordenados([], 'es')).toEqual([]);
	});
});

describe('sinAcentos', () => {
	test('saca las tildes y baja las mayúsculas', () => {
		expect(sinAcentos('Pérez')).toBe('perez');
		expect(sinAcentos('Núñez')).toBe('nunez');
		expect(sinAcentos('ÁÉÍÓÚ')).toBe('aeiou');
	});

	test('lo que no tiene acentos queda igual', () => {
		expect(sinAcentos('Ana')).toBe('ana');
		expect(sinAcentos('')).toBe('');
	});
});

describe('buscados', () => {
	const agenda = [
		contacto('Ana Pérez', {
			orden: 'Pérez, Ana',
			organizacion: 'Vasak Group',
			correos: [{ tipo: 'work', valor: 'ana@vasak.net.ar' }],
			telefonos: [{ tipo: 'cell', valor: '+54 11 5555-1234' }],
		}),
		contacto('Juan Gómez', {
			orden: 'Gómez, Juan',
			organizacion: 'Panadería del barrio',
			correos: [{ tipo: 'home', valor: 'juan@ejemplo.com' }],
		}),
	];

	test('buscar sin acentos encuentra igual', () => {
		// Nadie escribe los acentos en un buscador, ni siquiera quien los
		// escribe bien en todo lo demás.
		expect(buscados(agenda, 'perez')).toHaveLength(1);
		expect(buscados(agenda, 'Perez')[0].nombre).toBe('Ana Pérez');
		expect(buscados(agenda, 'gomez')[0].nombre).toBe('Juan Gómez');
	});

	test('busca también por organización y por correo', () => {
		// La gente busca por lo que recuerda, y a veces lo que recuerda es «el
		// que trabaja en Vasak».
		expect(buscados(agenda, 'vasak')[0].nombre).toBe('Ana Pérez');
		expect(buscados(agenda, 'panaderia')[0].nombre).toBe('Juan Gómez');
		expect(buscados(agenda, 'juan@ejemplo')[0].nombre).toBe('Juan Gómez');
	});

	test('un teléfono se encuentra sin escribir los separadores', () => {
		// Nadie busca «11-5555» poniendo el guión en el mismo lugar.
		expect(buscados(agenda, '5555 1234')).toHaveLength(1);
		expect(buscados(agenda, '55551234')).toHaveLength(1);
		expect(buscados(agenda, '999999')).toHaveLength(0);
	});

	test('cada palabra puede aparecer en una parte distinta', () => {
		// «ana vasak» encuentra a Ana que trabaja en Vasak, que es lo que
		// quiere decir quien lo escribe.
		expect(buscados(agenda, 'ana vasak')).toHaveLength(1);
		// Y dos palabras que no están en el mismo contacto no encuentran nada,
		// aunque cada una por su lado encuentre a alguien.
		expect(buscados(agenda, 'perez panaderia')).toHaveLength(0);
	});

	test('busca por partes de una palabra, a propósito', () => {
		// Escribir «ana» encuentra a Susana, que es lo que se espera de un
		// buscador de nombres — y también a la panadería, porque «ana» está
		// adentro de «panadería». Ese ruido es el precio de que buscar por la
		// mitad de un nombre funcione, y es el trato que hace cualquier agenda.
		const conSusana = [...agenda, contacto('Susana Ruiz', { orden: 'Ruiz, Susana' })];
		const encontrados = buscados(conSusana, 'ana').map((c) => c.nombre);

		expect(encontrados).toContain('Susana Ruiz');
		expect(encontrados).toContain('Ana Pérez');
	});

	test('sin consulta se devuelve todo', () => {
		expect(buscados(agenda, '')).toHaveLength(2);
		expect(buscados(agenda, '   ')).toHaveLength(2);
	});

	test('lo que no está no aparece', () => {
		expect(buscados(agenda, 'nadie')).toHaveLength(0);
	});
});

describe('inicialDe', () => {
	test('agrupa sin mirar el acento', () => {
		// Álvarez y Alvarez en la misma letra, no en dos grupos separados por
		// todo el alfabeto.
		expect(inicialDe(contacto('x', { orden: 'Álvarez, Ana' }))).toBe('A');
		expect(inicialDe(contacto('x', { orden: 'Alvarez, Ana' }))).toBe('A');
		expect(inicialDe(contacto('x', { orden: 'ñandú' }))).toBe('N');
	});

	test('lo que no empieza con una letra va a su propio grupo', () => {
		// Mezclarlo con la A es peor que darle el suyo.
		expect(inicialDe(contacto('x', { orden: '3 Amigos' }))).toBe('#');
		expect(inicialDe(contacto('x', { orden: '' }))).toBe('#');
		expect(inicialDe(contacto('x', { orden: '   ' }))).toBe('#');
		expect(inicialDe(contacto('x', { orden: '日本' }))).toBe('#');
	});
});
