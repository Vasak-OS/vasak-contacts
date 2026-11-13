/**
 * Exportar la agenda a CSV (vasak-contacts#9): el texto que sale de
 * `contactsToCsv` y el cableado del botón de la ventana.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { ActionButton } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { emptyContact } from '@/tools/address-book';
import { contactsToCsv } from '@/tools/csv';
import AddressBookView from '@/views/AddressBookView.vue';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

const ENCABEZADO =
	'Name,Nickname,Organization,Title,Emails,Phones,Addresses,Websites,Birthday,Categories,Notes';

/** Las filas de datos, sin el encabezado, ya separadas. */
const filas = (csv: string) => csv.split('\r\n').slice(1);

describe('contactsToCsv', () => {
	test('lleva el encabezado y una fila por contacto, separadas por CRLF', () => {
		const csv = contactsToCsv([
			emptyContact({ name: 'Ana' }),
			emptyContact({ name: 'Bruno' }),
		]);

		const lineas = csv.split('\r\n');
		expect(lineas[0]).toBe(ENCABEZADO);
		expect(lineas).toHaveLength(3);
		expect(lineas[1]?.startsWith('Ana,')).toBe(true);
		expect(lineas[2]?.startsWith('Bruno,')).toBe(true);
	});

	test('sin contactos sale sólo el encabezado', () => {
		expect(contactsToCsv([])).toBe(ENCABEZADO);
	});

	test('un campo con coma, comillas o salto de línea va entre comillas y las comillas se duplican', () => {
		const csv = contactsToCsv([
			emptyContact({ name: 'Pérez, Ana', nickname: 'A"B', organization: 'Línea\nuno' }),
		]);

		expect(filas(csv)[0]?.startsWith('"Pérez, Ana","A""B","Línea\nuno",')).toBe(true);
	});

	test('una nota de varios renglones queda en una sola celda entrecomillada', () => {
		const csv = contactsToCsv([emptyContact({ name: 'Ana', notes: 'uno\r\ndos\ntres' })]);

		expect(csv.endsWith(',"uno\r\ndos\ntres"')).toBe(true);
	});

	test('varios valores van en una celda, separados por « ::: »', () => {
		const csv = contactsToCsv([
			emptyContact({
				name: 'Ana',
				emails: [
					{ label: '', value: 'a@x.com' },
					{ label: '', value: 'b@x.com' },
				],
				phones: [
					{ label: '', value: '111' },
					{ label: '', value: '222' },
				],
				websites: [
					{ label: '', value: 'https://a.com' },
					{ label: '', value: 'https://b.com' },
				],
				categories: ['amigos', 'trabajo'],
			}),
		]);

		const fila = filas(csv)[0] ?? '';
		expect(fila).toContain('a@x.com ::: b@x.com');
		expect(fila).toContain('111 ::: 222');
		expect(fila).toContain('https://a.com ::: https://b.com');
		expect(fila).toContain('amigos ::: trabajo');
	});

	test('el cumpleaños completo sale como AAAA-MM-DD; el parcial, como vino; sin fecha, vacío', () => {
		const completo = contactsToCsv([
			emptyContact({ birthday: { year: 1990, month: 3, day: 7, text: '' } }),
		]);
		const parcial = contactsToCsv([
			emptyContact({ birthday: { year: null, month: 3, day: 7, text: '7 de marzo' } }),
		]);
		const sin = contactsToCsv([emptyContact()]);

		expect(filas(completo)[0]).toContain(',1990-03-07,');
		expect(filas(parcial)[0]).toContain(',7 de marzo,');
		expect(filas(sin)[0]).toBe(',,,,,,,,,,');
	});

	test('un campo que empieza con = + - @ tab o CR se neutraliza con un apóstrofo', () => {
		for (const peligroso of ['=HYPERLINK("http://x")', '+1', '-2', '@SUMA(A1)', '\tx', '\rx']) {
			const csv = contactsToCsv([emptyContact({ name: peligroso })]);
			const celda = filas(csv)[0] ?? '';
			// El apóstrofo va pegado al primer carácter (dentro de las comillas, si las hay).
			expect(celda.replace(/^"/, '').startsWith(`'${peligroso[0]}`)).toBe(true);
		}
	});

	test('la fórmula neutralizada conserva el resto del texto, con su entrecomillado', () => {
		const csv = contactsToCsv([emptyContact({ name: '=HYPERLINK("http://x","y")' })]);

		expect(filas(csv)[0]?.startsWith(`"'=HYPERLINK(""http://x"",""y"")",`)).toBe(true);
	});

	test('los encabezados no se neutralizan y un texto común no lleva apóstrofo', () => {
		const csv = contactsToCsv([emptyContact({ name: 'Ana' })]);

		expect(csv.split('\r\n')[0]).toBe(ENCABEZADO);
		expect(filas(csv)[0]?.startsWith('Ana,')).toBe(true);
		expect(csv).not.toContain("'");
	});
});

describe('el botón «Exportar a CSV»', () => {
	const ANA = emptyContact({ name: 'Ana Pérez', sort_key: 'Pérez, Ana', url: 'store:a1:/ana.vcf' });
	const BRUNO = emptyContact({ name: 'Bruno', sort_key: 'Bruno', url: 'store:a1:/bruno.vcf' });
	let mounted: VueWrapper | null = null;

	afterEach(() => {
		mounted?.unmount();
		mounted = null;
		olvidarTodo();
		document.body.innerHTML = '';
	});

	async function exportarCon(ruta: string | null) {
		answerCommand('listar_cuentas', () => [
			{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
		]);
		answerCommand('contactos_de_la_cuenta', () => ({
			libretas: [],
			contactos: [ANA, BRUNO],
			fallos: [],
		}));
		// `save()` del plugin de diálogos pide la ruta por este comando.
		answerCommand('plugin:dialog|save', () => ruta);
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();
		await mounted
			.findAllComponents(ActionButton)
			.find((b) => b.attributes('data-testid') === 'export-csv')
			?.trigger('click');
		await flushPromises();
		return invokedCommands.filter((c) => c.command === 'exportar_csv');
	}

	test('escribe lo que se ve en la ruta que eligió la persona', async () => {
		const llamadas = await exportarCon('/home/ana/contactos.csv');

		expect(llamadas).toHaveLength(1);
		expect(llamadas[0]?.args?.ruta).toBe('/home/ana/contactos.csv');
		const csv = String(llamadas[0]?.args?.csv);
		expect(csv.split('\r\n')[0]).toBe(ENCABEZADO);
		expect(csv).toContain('Ana Pérez');
		expect(csv).toContain('Bruno');
	});

	test('si la persona cancela el diálogo no se escribe nada', async () => {
		expect(await exportarCon(null)).toHaveLength(0);
	});
});

describe('el botón «Exportar a vCard»', () => {
	let mounted: VueWrapper | null = null;

	afterEach(() => {
		mounted?.unmount();
		mounted = null;
		olvidarTodo();
		document.body.innerHTML = '';
	});

	test('pide la cuenta entera al programa, sin armar nada en la ventana', async () => {
		answerCommand('listar_cuentas', () => [
			{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
		]);
		answerCommand('contactos_de_la_cuenta', () => ({
			libretas: [],
			contactos: [emptyContact({ name: 'Ana', account_id: 'a1', url: 'store:a1:/ana.vcf' })],
			fallos: [],
		}));
		answerCommand('plugin:dialog|save', () => '/home/ana/contactos.vcf');
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();
		await mounted
			.findAllComponents(ActionButton)
			.find((b) => b.attributes('data-testid') === 'export-vcf')
			?.trigger('click');
		await flushPromises();

		const llamadas = invokedCommands.filter((c) => c.command === 'exportar_vcf');
		expect(llamadas).toHaveLength(1);
		expect(llamadas[0]?.args).toEqual({
			ruta: '/home/ana/contactos.vcf',
			accountId: 'a1',
			addressBookId: '',
		});
	});

	test('muestra un aviso propio cuando el programa devuelve la marca de que no hay contactos', async () => {
		answerCommand('listar_cuentas', () => [
			{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
		]);
		answerCommand('contactos_de_la_cuenta', () => ({
			libretas: [],
			contactos: [emptyContact({ name: 'Ana', account_id: 'a1', url: 'store:a1:/ana.vcf' })],
			fallos: [],
		}));
		answerCommand('plugin:dialog|save', () => '/home/ana/contactos.vcf');
		answerCommand('exportar_vcf', () => {
			throw 'export-no-contacts';
		});
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();
		await mounted
			.findAllComponents(ActionButton)
			.find((b) => b.attributes('data-testid') === 'export-vcf')
			?.trigger('click');
		await flushPromises();

		expect(mounted.text()).not.toContain('export-no-contacts');
	});
});
