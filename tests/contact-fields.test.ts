import { describe, expect, test } from 'bun:test';
import type { Address } from '../src/tools/address-book';
import {
	addressLines,
	customFieldName,
	formatPartialDate,
	initialsOf,
	labelKey,
	languageName,
	serviceName,
	webLink,
} from '../src/tools/contact-fields';

function address(parts: Partial<Address>): Address {
	return {
		label: '',
		po_box: '',
		extended: '',
		street: '',
		locality: '',
		region: '',
		postal_code: '',
		country: '',
		...parts,
	};
}

describe('addressLines', () => {
	test('cada parte en su renglón, en el orden en que se lee', () => {
		expect(
			addressLines(
				address({
					street: 'Av. Siempreviva 742',
					extended: '3° B',
					locality: 'Springfield',
					region: 'Buenos Aires',
					postal_code: 'B1636',
					country: 'Argentina',
				})
			)
		).toEqual(['Av. Siempreviva 742', '3° B', 'Springfield', 'Buenos Aires', 'B1636', 'Argentina']);
	});

	test('los campos vacíos no dejan renglones vacíos', () => {
		// Una dirección con sólo la ciudad es de lo más común: la agenda del
		// teléfono deja el resto en blanco.
		expect(addressLines(address({ locality: 'Springfield' }))).toEqual(['Springfield']);
		expect(addressLines(address({ street: '  ', country: 'Chile' }))).toEqual(['Chile']);
		expect(addressLines(address({}))).toEqual([]);
	});

	test('el apartado de correos va primero', () => {
		expect(addressLines(address({ po_box: 'Casilla 123', locality: 'Lima' }))).toEqual([
			'Casilla 123',
			'Lima',
		]);
	});
});

describe('formatPartialDate', () => {
	const date = (year: number | null, month: number | null, day: number | null) => ({
		year,
		month,
		day,
		text: '',
	});

	test('una fecha completa, en el idioma de la sesión', () => {
		expect(formatPartialDate(date(1985, 4, 15), 'es')).toBe('15 de abril de 1985');
		expect(formatPartialDate(date(1985, 4, 15), 'en')).toBe('April 15, 1985');
	});

	test('sin año no se inventa uno', () => {
		// `new Date(mes, día)` pondría 1970, o el año en curso: las dos cosas
		// dicen algo que la tarjeta no dijo.
		const shown = formatPartialDate(date(null, 4, 15), 'es');
		expect(shown).toBe('15 de abril');
		expect(shown).not.toMatch(/\d{4}/);
	});

	test('el 29 de febrero sin año existe', () => {
		expect(formatPartialDate(date(null, 2, 29), 'es')).toBe('29 de febrero');
	});

	test('no se corre un día por la zona horaria', () => {
		// La fecha no tiene hora. Armada en la zona de la sesión, al oeste de
		// Greenwich el 1 de enero salía 31 de diciembre.
		expect(formatPartialDate(date(2000, 1, 1), 'en')).toBe('January 1, 2000');
	});

	test('las fechas a medias de la 4.0', () => {
		expect(formatPartialDate(date(1985, 4, null), 'es')).toBe('abril de 1985');
		expect(formatPartialDate(date(1985, null, null), 'es')).toBe('1985');
		expect(formatPartialDate(date(null, 4, null), 'es')).toBe('abril');
		expect(formatPartialDate(date(null, null, 15), 'es')).toBe('15');
	});

	test('lo que no era una fecha se muestra como vino', () => {
		expect(formatPartialDate({ year: null, month: null, day: null, text: 'circa 1800' }, 'es')).toBe(
			'circa 1800'
		);
	});
});

describe('webLink', () => {
	test('una web de verdad se abre', () => {
		expect(webLink('https://ana.ejemplo.com')).toBe('https://ana.ejemplo.com/');
		expect(webLink('http://ana.ejemplo.com/blog')).toBe('http://ana.ejemplo.com/blog');
	});

	test('sin esquema se abre por https, que es como la escribe mucha gente', () => {
		expect(webLink('ana.ejemplo.com')).toBe('https://ana.ejemplo.com/');
		expect(webLink('  www.ejemplo.com/ana  ')).toBe('https://www.ejemplo.com/ana');
	});

	test('lo que no es una web no se abre nunca', () => {
		// La tarjeta la escribió cualquiera: abrir con el sistema algo que no es
		// `http` hace otra cosa que mostrar una página.
		for (const dangerous of [
			'javascript:alert(1)',
			'file:///etc/passwd',
			'smb://servidor/compartido',
			'mailto:ana@ejemplo.com',
			'data:text/html,<script>',
			'https://ana@ejemplo.com',
			'https://a b.com',
			'https://ejemplo.com/\nsiguiente',
			'',
			'   ',
		]) {
			expect(webLink(dangerous)).toBe('');
		}
	});
});

describe('initialsOf', () => {
	test('la primera y la última palabra', () => {
		expect(initialsOf('Ana Pérez')).toBe('AP');
		expect(initialsOf('Ana María Pérez')).toBe('AP');
		expect(initialsOf('ana')).toBe('A');
	});

	test('las letras con acento y fuera del plano básico no se parten', () => {
		expect(initialsOf('Ángel Ñandú')).toBe('ÁÑ');
		expect(initialsOf('𝒜na Pérez')).toBe('𝒜P');
	});

	test('sin nombre no hay iniciales', () => {
		expect(initialsOf('')).toBe('');
		expect(initialsOf('   ')).toBe('');
		expect(initialsOf('(trabajo) 3M')).toBe('');
	});
});

describe('languageName', () => {
	test('el nombre del idioma en el de la sesión', () => {
		// La mayúscula depende de los datos de idioma del motor, no de esto.
		expect(languageName('es-AR', 'es').toLowerCase()).toBe('español (argentina)');
		expect(languageName('en', 'en')).toBe('English');
	});

	test('una etiqueta que no se entiende se muestra como vino', () => {
		expect(languageName('no válida', 'es')).toBe('no válida');
	});
});

describe('customFieldName', () => {
	test('la etiqueta que escribió la persona, si la hay', () => {
		expect(customFieldName({ name: 'X-CUSTOM', label: 'color favorito', value: 'azul' })).toBe(
			'color favorito'
		);
	});

	test('si no, el nombre de la propiedad sin el X- y con espacios', () => {
		expect(customFieldName({ name: 'X-PHONETIC-FIRST-NAME', label: '', value: 'A-na' })).toBe(
			'Phonetic first name'
		);
		expect(customFieldName({ name: 'X-', label: '', value: 'v' })).toBe('X-');
	});
});

describe('labelKey', () => {
	test('las etiquetas de siempre se traducen', () => {
		expect(labelKey('home')).toBe('contact.labels.home');
		expect(labelKey('WORK')).toBe('contact.labels.work');
		expect(labelKey('iPhone')).toBe('contact.labels.cell');
	});

	test('las que escribió la persona no', () => {
		expect(labelKey('casa de la playa')).toBeNull();
		expect(labelKey('')).toBeNull();
	});
});

describe('serviceName', () => {
	test('las marcas se escriben como se escriben', () => {
		expect(serviceName('linkedin')).toBe('LinkedIn');
		expect(serviceName('xmpp')).toBe('XMPP');
		expect(serviceName('Skype')).toBe('Skype');
	});

	test('un servicio que no se conoce, con la primera en mayúscula', () => {
		expect(serviceName('pixelfed')).toBe('Pixelfed');
		expect(serviceName('')).toBe('');
	});
});
