/**
 * Las piezas de los contactos que pasaron a la librería (vue-libvasak#74).
 *
 * Lo que se comprueba no es cómo se dibuja cada componente —eso se prueba en
 * la librería— sino que los contactos los usen y les pasen lo suyo: las tres
 * columnas son `Panel`, los títulos `SectionHeading`, las filas de la lista
 * `ListRow` con lo elegido marcado, los botones de la ficha `ActionButton` y
 * las categorías `Badge`. Y que nada de eso cambie lo que hace cada cosa: que
 * elegir un contacto siga avisando y que «Escribir» siga abriendo el correo.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import {
	ActionButton,
	Badge,
	EmptyState,
	ListRow,
	LoadingState,
	Panel,
	SectionHeading,
	StatusDot,
} from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import AccountsPanel from '@/components/contacts/AccountsPanel.vue';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import ContactList from '@/components/contacts/ContactList.vue';
import { type Contact, emptyContact } from '@/tools/address-book';
import AddressBookView from '@/views/AddressBookView.vue';
import { invokedCommands, olvidarTodo } from './dobles';

let mounted: VueWrapper | null = null;

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
});

const ANA = emptyContact({ name: 'Ana Pérez', sort_key: 'Pérez, Ana', url: 'https://x/ana.vcf', organization: 'Estudio Sur' });
const BRUNO = emptyContact({ name: 'Bruno Álvarez', sort_key: 'Álvarez, Bruno', url: 'https://x/bruno.vcf' });

function list(props: Record<string, unknown> = {}) {
	mounted = mount(ContactList, {
		props: { contacts: [BRUNO, ANA], selected: null, query: '', loading: false, ...props },
	});
	return mounted;
}

function detail(contact: Partial<Contact>) {
	mounted = mount(ContactDetail, {
		props: { contact: emptyContact({ name: 'Ana Pérez', url: 'https://x/ana.vcf', ...contact }) },
	});
	return mounted;
}

describe('las tres columnas', () => {
	test('son `Panel` de la librería', () => {
		mounted = mount(AddressBookView);

		expect(mounted.findAllComponents(Panel)).toHaveLength(3);
	});

	test('y las dos de la izquierda ceden ancho en una ventana angosta', () => {
		// Con el ancho fijo solo, por debajo de unos 700 px la ficha no tenía
		// lugar. Desde unos 830 px de fila miden lo mismo que antes.
		mounted = mount(AddressBookView);
		const [accounts, contacts] = mounted.findAllComponents(Panel);

		expect(accounts?.classes()).toContain('max-w-[25%]');
		expect(contacts?.classes()).toContain('max-w-[35%]');
	});

	test('actualizar es el `ActionButton` sin relleno, como los de la ventana', () => {
		mounted = mount(AddressBookView);
		const refresh = mounted.findAllComponents(ActionButton).find((b) => b.props('icon') === 'view-refresh');

		expect(refresh?.props('variant')).toBe('ghost');
		expect(refresh?.attributes('aria-label')).toBe('lista.actualizar');
	});
});

describe('el panel de cuentas', () => {
	function accounts(props: Record<string, unknown> = {}) {
		mounted = mount(AccountsPanel, {
			props: {
				accounts: [
					{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
					{ id: 'a2', nombre: 'trabajo', necesita_reconectarse: true },
				],
				notices: [],
				...props,
			},
		});
		return mounted;
	}

	test('el título es `SectionHeading`', () => {
		expect(accounts().findComponent(SectionHeading).props('title')).toBe('cuentas.titulo');
	});

	test('una cuenta por reconectar lo dice con el punto de aviso y texto legible', () => {
		const note = accounts().find('[data-testid="needs-reconnect"]');

		expect(note.findComponent(StatusDot).props('tone')).toBe('warning');
		expect(note.classes()).not.toContain('text-status-warning');
	});

	test('sin cuentas, el vacío de la librería dice qué hacer', () => {
		const empty = accounts({ accounts: [] }).findComponent(EmptyState);

		expect(empty.props('title')).toBe('cuentas.sinCuentasTitulo');
		expect(empty.props('note')).toBe('cuentas.sinCuentasDescripcion');
	});
});

describe('la lista', () => {
	test('cada contacto es un `ListRow` que se puede apretar', () => {
		const rows = list().findAllComponents(ListRow);

		expect(rows.map((r) => r.props('title'))).toEqual(['Bruno Álvarez', 'Ana Pérez']);
		expect(rows.every((r) => r.props('role') === 'button')).toBe(true);
	});

	test('la organización va de descripción, y sin ella no queda un renglón vacío', () => {
		const [bruno, ana] = list().findAllComponents(ListRow);

		expect(ana?.props('description')).toBe('Estudio Sur');
		expect(bruno?.props('description')).toBeUndefined();
	});

	test('el que se está viendo va marcado como elegido', () => {
		const rows = list({ selected: ANA }).findAllComponents(ListRow);

		expect(rows.map((r) => r.props('selected'))).toEqual([false, true]);
		expect(rows[1]?.attributes('aria-current')).toBe('true');
	});

	test('apretar una fila avisa cuál', async () => {
		const view = list();
		await view.findAllComponents(ListRow)[1]?.trigger('click');

		expect(view.emitted('select')?.[0]).toEqual([ANA]);
	});

	test('y también con el teclado', async () => {
		const view = list();
		await view.findAllComponents(ListRow)[0]?.trigger('keydown', { key: 'Enter' });

		expect(view.emitted('select')?.[0]).toEqual([BRUNO]);
	});

	test('las iniciales son `SectionHeading` pegados arriba, sobre el panel', () => {
		const headings = list().findAllComponents(SectionHeading);

		expect(headings.map((h) => h.props('title'))).toEqual(['A', 'P']);
		expect(headings.every((h) => h.props('sticky') && h.props('surface') === 'panel')).toBe(true);
	});

	test('cargando, la fila chica de la librería', () => {
		const loading = list({ contacts: [], loading: true }).findComponent(LoadingState);

		expect(loading.props('label')).toBe('lista.cargando');
		expect(loading.props('size')).toBe('sm');
	});

	test('sin resultados, el vacío dice qué se buscó', () => {
		// Sin catálogo, `t()` devuelve la clave, y la clave no tiene `{0}`: lo
		// que se comprueba es que sea el texto de «sin resultados» y no el de la
		// libreta vacía.
		const empty = list({ contacts: [], query: 'pepe' }).findComponent(EmptyState);

		expect(empty.props('title')).toBe('lista.sinResultados');
	});
});

describe('la ficha', () => {
	test('los títulos de los tramos son `SectionHeading`', () => {
		const view = detail({ emails: [{ label: '', value: 'ana@x.example' }], notes: 'hola' });

		expect(view.findAllComponents(SectionHeading).map((h) => h.props('title'))).toEqual([
			'contact.emails',
			'contact.notes',
		]);
	});

	test('las categorías son insignias de contorno', () => {
		const badges = detail({ categories: ['Trabajo', 'Amigos'] }).findAllComponents(Badge);

		expect(badges.map((b) => b.props('label'))).toEqual(['Trabajo', 'Amigos']);
		expect(badges.every((b) => b.props('variant') === 'outline')).toBe(true);
	});

	test('los botones son `ActionButton` chicos y sin relleno', () => {
		const buttons = detail({
			emails: [{ label: '', value: 'ana@x.example' }],
			phones: [{ label: '', value: '+54 11 5555-0101' }],
		}).findAllComponents(ActionButton);

		expect(buttons.map((b) => b.props('label'))).toEqual([
			// El de volver a la lista, que sólo se ve con la ventana angosta.
			'nav.contacts',
			'contact.write',
			'contact.copy',
			'contact.call',
			'contact.copy',
		]);
		expect(buttons.every((b) => b.props('variant') === 'ghost' && b.props('size') === 'sm')).toBe(true);
	});

	test('se acomoda por el ancho de la ficha, no por el de la pantalla', () => {
		// En unos 130 px de ficha la foto va arriba del nombre y los botones
		// abajo del valor; desde 12rem, como siempre. Con consultas de
		// contenedor: en WebKitGTK ni `matchMedia` ni `resize` avisan.
		const view = detail({ emails: [{ label: '', value: 'ana@x.example' }] });

		expect(view.findComponent(Panel).classes()).toContain('@container');
		expect(view.find('header').classes()).toEqual(
			expect.arrayContaining(['flex-col', '@min-[12rem]:flex-row'])
		);
		const write = view.findAllComponents(ActionButton).find((b) => b.props('label') === 'contact.write');
		const row = write?.element.parentElement;
		expect(row?.className).toContain('@max-[12rem]:flex-wrap');
	});

	test('«Escribir» sigue abriendo el correo con el sistema', async () => {
		const view = detail({ emails: [{ label: '', value: 'ana@x.example' }] });
		await view
			.findAllComponents(ActionButton)
			.find((b) => b.props('label') === 'contact.write')
			?.trigger('click');
		await flushPromises();

		const opened = invokedCommands.filter((c) => c.command === 'plugin:shell|open').map((c) => c.args?.path);
		// Codificada: ver `enlaces.ts`.
		expect(opened).toEqual(['mailto:ana%40x.example']);
	});
});
