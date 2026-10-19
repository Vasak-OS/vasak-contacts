import { describe, expect, test } from 'bun:test';
// Se importa el módulo entero y no los nombres: contra el código viejo, que no
// tiene `searchMatches` ni `phoneticKey`, cada prueba falla por su cuenta en
// vez de que el archivo entero se caiga al importar.
import * as agenda from '../src/tools/address-book';
import { type Contact, emptyContact } from '../src/tools/address-book';

function contact(name: string, extra: Partial<Contact> = {}): Contact {
	return emptyContact({ uid: name, name, sort_key: name, ...extra });
}

const nombres = (lista: Contact[]) => lista.map((c) => c.name);

describe('búsqueda, fonética en español', () => {
	test('«gonsález» encuentra a «Gonzalez»', () => {
		const lista = [contact('Ana Gonzalez'), contact('Luis Martinez')];
		expect(nombres(agenda.searchContacts(lista, 'gonsález'))).toEqual(['Ana Gonzalez']);
	});

	test('«stephen» encuentra a «Steven»', () => {
		const lista = [contact('Steven Rojas'), contact('Luis Martinez')];
		expect(nombres(agenda.searchContacts(lista, 'stephen'))).toEqual(['Steven Rojas']);
	});

	test('v y b suenan igual', () => {
		const lista = [contact('Carlos Vargas'), contact('Luis Martinez')];
		expect(nombres(agenda.searchContacts(lista, 'bargas'))).toEqual(['Carlos Vargas']);
	});

	test('g ante e/i suena como j', () => {
		const lista = [contact('Marta Gimenez'), contact('Luis Martinez')];
		expect(nombres(agenda.searchContacts(lista, 'jimenez'))).toEqual(['Marta Gimenez']);
	});

	test('z y s suenan igual', () => {
		const lista = [contact('Pedro Zapata'), contact('Luis Martinez')];
		expect(nombres(agenda.searchContacts(lista, 'sapata'))).toEqual(['Pedro Zapata']);
	});

	test('la g dura de «gue/gui» no se confunde con la j', () => {
		// Sin esa excepción «Guerra» y «Jerra» sonarían igual.
		expect(agenda.phoneticKey('guerra')).not.toBe(agenda.phoneticKey('jerra'));
		expect(agenda.phoneticKey('gato')).not.toBe(agenda.phoneticKey('jato'));
	});

	test('las demás reglas de oído: ll, h muda, qu, ch y c ante e/i', () => {
		expect(agenda.phoneticKey('llama')).toBe(agenda.phoneticKey('yama'));
		expect(agenda.phoneticKey('hugo')).toBe(agenda.phoneticKey('ugo'));
		expect(agenda.phoneticKey('quique')).toBe(agenda.phoneticKey('kike'));
		expect(agenda.phoneticKey('cesar')).toBe(agenda.phoneticKey('sesar'));
	});

	test('un nombre que no suena parecido no aparece', () => {
		const lista = [contact('Ana Gonzalez'), contact('Luis Martinez')];
		expect(agenda.searchContacts(lista, 'gonsález rodriguez')).toHaveLength(0);
		expect(agenda.searchContacts(lista, 'fernandez')).toHaveLength(0);
	});

	test('levenshtein cuenta las ediciones', () => {
		expect(agenda.levenshtein('', 'abc')).toBe(3);
		expect(agenda.levenshtein('casa', 'casa')).toBe(0);
		expect(agenda.levenshtein('casa', 'caso')).toBe(1);
		expect(agenda.levenshtein('kitten', 'sitting')).toBe(3);
	});

	test('una palabra de tres letras no tolera errores', () => {
		const lista = [contact('Ana Rojas')];
		// «ani» está a una edición de «ana», pero en una palabra corta eso es
		// demasiado ruido.
		expect(agenda.searchContacts(lista, 'ani')).toHaveLength(0);
	});
});

describe('búsqueda, en qué campos mira', () => {
	const lista = [
		contact('Ana Rojas', {
			organization: 'Cooperativa Quimera',
			title: 'Ingeniera',
			role: 'Coordinadora',
			addresses: [
				{
					label: 'home',
					po_box: '',
					extended: '',
					street: 'Avenida Corrientes 1234',
					locality: 'Rosario',
					region: 'Santa Fe',
					postal_code: 'S2000',
					country: 'Argentina',
				},
			],
			categories: ['Vecinos'],
			notes: 'Le gusta el mate amargo',
		}),
		contact('Luis Martinez'),
	];

	test.each([
		['organización', 'quimera'],
		['cargo', 'ingeniera'],
		['rol', 'coordinadora'],
		['calle', 'corrientes'],
		['localidad', 'rosario'],
		['provincia', 'santa fe'],
		['código postal', 's2000'],
		['país', 'argentina'],
		['etiqueta', 'vecinos'],
		['notas', 'amargo'],
	])('una coincidencia en %s encuentra el contacto', (_campo, consulta) => {
		expect(nombres(agenda.searchContacts(lista, consulta))).toEqual(['Ana Rojas']);
	});

	test('cada palabra puede estar en un campo distinto', () => {
		expect(nombres(agenda.searchContacts(lista, 'ana quimera rosario'))).toEqual(['Ana Rojas']);
		expect(agenda.searchContacts(lista, 'ana inexistente')).toHaveLength(0);
	});

	test('sin consulta se devuelve toda la agenda', () => {
		expect(agenda.searchContacts(lista, '  ')).toHaveLength(2);
		expect(agenda.searchMatches(lista, '')).toHaveLength(2);
	});
});

describe('búsqueda, las dos trampas de los teléfonos', () => {
	const lista = [
		contact('Bruno', {
			phones: [{ label: 'cell', value: '+54 11 5555-1234' }],
			emails: [{ label: 'work', value: 'bruno@ejemplo.com' }],
		}),
		contact('Carla', { emails: [{ label: 'work', value: 'carla@ejemplo.com' }] }),
	];

	test('un teléfono se encuentra por una parte de sus dígitos', () => {
		expect(nombres(agenda.searchContacts(lista, '551234'))).toEqual(['Bruno']);
		expect(nombres(agenda.searchContacts(lista, '541155551234'))).toEqual(['Bruno']);
		expect(nombres(agenda.searchContacts(lista, '(11) 5555-1234'))).toEqual(['Bruno']);
	});

	test('una palabra sin dígitos que no está no devuelve la agenda entera', () => {
		// Sin dígitos queda la cadena vacía y todo texto la contiene.
		expect(agenda.searchContacts(lista, 'nadie')).toHaveLength(0);
		expect(agenda.searchContacts(lista, 'qqq')).toHaveLength(0);
		expect(agenda.searchContacts(lista, 'ana nadie')).toHaveLength(0);
	});

	test('los dígitos de una palabra mixta no coinciden con campos que no son teléfonos', () => {
		// «ana1» tiene un 1; el contacto no tiene teléfono y sí un «1» en la
		// dirección y en un correo. Antes los dígitos se comparaban contra todo.
		const sinTelefono = [
			contact('Dora', {
				emails: [{ label: 'work', value: 'dora1@ejemplo.com' }],
				addresses: [
					{
						label: 'home',
						po_box: '',
						extended: '',
						street: 'Calle 1',
						locality: '',
						region: '',
						postal_code: '',
						country: '',
					},
				],
			}),
		];
		expect(agenda.searchContacts(sinTelefono, 'ana1')).toHaveLength(0);
	});

	test('los dígitos de una dirección no se toman por un teléfono', () => {
		const conCalle = [
			contact('Dora', {
				addresses: [
					{
						label: 'home',
						po_box: '',
						extended: '',
						street: 'Avenida 5551234',
						locality: '',
						region: '',
						postal_code: '',
						country: '',
					},
				],
			}),
		];
		// «5551-234» sólo coincide por dígitos sueltos, y esos son de teléfonos.
		expect(agenda.searchContacts(conCalle, '5551-234')).toHaveLength(0);
	});
});

describe('búsqueda, orden por calidad de coincidencia', () => {
	test('exacta, luego prefijo, luego subcadena, luego fonética', () => {
		// Se arma en el orden contrario al esperado: si no ordena, falla.
		const lista = [
			contact('Pata Rios'), // fonética (a una edición de «pato»)
			contact('Mapato Ruiz'), // subcadena
			contact('Patorrico Diaz'), // prefijo
			contact('Pato Gomez'), // exacta
		];
		const encontrados = agenda.searchMatches(lista, 'pato');
		expect(encontrados.map((m) => m.contact.name)).toEqual([
			'Pato Gomez',
			'Patorrico Diaz',
			'Mapato Ruiz',
			'Pata Rios',
		]);
		expect(encontrados.map((m) => m.score)).toEqual([4, 3, 2, 1]);
	});

	test('a igual puntaje conserva el orden de entrada', () => {
		const lista = [contact('Ana B'), contact('Ana A'), contact('Ana C')];
		expect(nombres(agenda.searchContacts(lista, 'ana'))).toEqual(['Ana B', 'Ana A', 'Ana C']);
	});

	test('el puntaje suma por palabra', () => {
		const lista = [
			contact('Ana Rojas', { organization: 'Vasak' }),
			contact('Ana Lopez', { organization: 'Otra' }),
		];
		// «ana» y «vasak» exactas = 8; la otra no tiene «vasak».
		const encontrados = agenda.searchMatches(lista, 'ana vasak');
		expect(encontrados.map((m) => [m.contact.name, m.score])).toEqual([['Ana Rojas', 8]]);
	});

	test('un apodo parecido («Pata» al buscar «pato») queda al final, no desaparece ni sube', () => {
		const lista = [
			contact('Joaquin', { nickname: 'Pata' }),
			contact('Zulema', { notes: 'amiga de pato' }),
			contact('Mario', { nickname: 'Pato' }),
			contact('Rita', { organization: 'Patos SA' }),
		];
		const encontrados = agenda.searchMatches(lista, 'pato');
		expect(encontrados.map((m) => m.contact.name)).toEqual(['Zulema', 'Mario', 'Rita', 'Joaquin']);
		expect(encontrados.at(-1)?.score).toBe(1);
		expect(Math.min(...encontrados.slice(0, -1).map((m) => m.score))).toBeGreaterThan(1);
	});
});

describe('búsqueda, privacidad de las notas', () => {
	test('searchMatches dice que coincidió en notas pero nunca da el texto', () => {
		const secreto = 'diagnostico confidencial del paciente';
		const lista = [contact('Ana Rojas', { notes: secreto })];
		const [encontrado] = agenda.searchMatches(lista, 'confidencial');

		expect(encontrado.fields).toEqual(['notes']);
		// Ni en `fields` ni en ningún otro valor de la coincidencia (salvo el
		// propio contacto, que es lo que se buscó) viaja un pedazo de la nota.
		const sinContacto = JSON.stringify({ score: encontrado.score, fields: encontrado.fields });
		expect(sinContacto).not.toContain('confidencial');
		expect(sinContacto).not.toContain('diagnostico');
		expect(Object.keys(encontrado).sort()).toEqual(['contact', 'fields', 'score']);
	});

	test('los campos son nombres de campo, de un conjunto cerrado', () => {
		const lista = [
			contact('Ana Rojas', {
				organization: 'Vasak',
				emails: [{ label: 'work', value: 'ana@vasak.net' }],
			}),
		];
		const [encontrado] = agenda.searchMatches(lista, 'ana vasak');
		const permitidos = [
			'name',
			'nickname',
			'organization',
			'title',
			'email',
			'phone',
			'address',
			'notes',
			'categories',
		];
		for (const campo of encontrado.fields) expect(permitidos).toContain(campo);
		expect(encontrado.fields).toContain('name');
	});
});
