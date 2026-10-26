/**
 * Las transiciones de la ficha y de la lista (vue-libvasak 2.21.0).
 *
 * Lo que se comprueba es la **estructura** de la que depende la animación,
 * porque happy-dom no anima ni calcula diseño: que haya una sola transición
 * con un hijo por rama, y que la ficha lleve una clave por contacto —eso es lo
 * que hace que cambiar de contacto anime y que refrescar al mismo no.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { EmptyState, ListRow, LoadingState, RevealTransition } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import ContactList from '@/components/contacts/ContactList.vue';
import { type Contact, emptyContact } from '@/tools/address-book';
import { olvidarTodo } from './dobles';

let mounted: VueWrapper | null = null;
let avisos: string[] = [];
const warn = console.warn;

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	console.warn = warn;
	avisos = [];
	olvidarTodo();
});

/** Lo que Vue avisa por consola: «Transition» con más de un hijo, por ejemplo. */
function anotarAvisos() {
	console.warn = (...args: unknown[]) => void avisos.push(args.join(' '));
}

const ANA = emptyContact({ name: 'Ana Pérez', sort_key: 'Pérez, Ana', url: 'https://x/ana.vcf' });
const BRUNO = emptyContact({ name: 'Bruno Álvarez', sort_key: 'Álvarez, Bruno', url: 'https://x/bruno.vcf' });

function list(props: Record<string, unknown> = {}) {
	anotarAvisos();
	mounted = mount(ContactList, {
		props: { contacts: [BRUNO, ANA], selected: null, query: '', loading: false, ...props },
	});
	return mounted;
}

function detail(contact: Contact | null) {
	anotarAvisos();
	mounted = mount(ContactDetail, { props: { contact } });
	return mounted;
}

describe('la lista, una transición entre estados', () => {
	test('con contactos hay una sola raíz de lista con todas las filas', () => {
		const view = list();

		expect(view.findAllComponents(RevealTransition)).toHaveLength(1);
		expect(view.findAllComponents(ListRow)).toHaveLength(2);
		expect(view.findComponent(LoadingState).exists()).toBe(false);
		expect(view.findComponent(EmptyState).exists()).toBe(false);
		expect(avisos).toEqual([]);
	});

	test('cargando sin contactos, sólo el estado de carga', () => {
		const view = list({ contacts: [], loading: true });

		expect(view.findComponent(LoadingState).exists()).toBe(true);
		expect(view.findComponent(EmptyState).exists()).toBe(false);
		expect(view.findAllComponents(ListRow)).toHaveLength(0);
	});

	test('cargando con contactos ya a la vista, la lista sigue', () => {
		const view = list({ loading: true });

		expect(view.findComponent(LoadingState).exists()).toBe(false);
		expect(view.findAllComponents(ListRow)).toHaveLength(2);
	});

	test('la libreta vacía dice que no hay nadie', () => {
		const empty = list({ contacts: [] }).findComponent(EmptyState);

		expect(empty.props('title')).toBe('lista.vacia');
	});

	test('una búsqueda sin resultados no es la libreta vacía', () => {
		const empty = list({ contacts: [], query: 'pepe' }).findComponent(EmptyState);

		expect(empty.props('title')).toBe('lista.sinResultados');
	});

	test('una búsqueda de puros espacios cuenta como libreta vacía', () => {
		const empty = list({ contacts: [], query: '   ' }).findComponent(EmptyState);

		expect(empty.props('title')).toBe('lista.vacia');
	});

	test('las filas siguen siendo accesibles dentro del contenedor nuevo', async () => {
		const view = list();
		const rows = view.findAllComponents(ListRow);

		expect(rows).toHaveLength(2);
		await rows[0]?.trigger('click');
		expect(view.emitted('select')?.[0]).toEqual([BRUNO]);
		expect(view.findAll('ul').length).toBeGreaterThan(0);
	});

	test('al pasar de estado queda un solo hijo vivo (sin avisos de Vue)', async () => {
		const view = list({ contacts: [], loading: true });
		await view.setProps({ contacts: [BRUNO, ANA], loading: false });
		await flushPromises();
		await view.setProps({ contacts: [], query: 'x' });
		await flushPromises();

		expect(avisos.filter((a) => /Transition|single element|multiple/i.test(a))).toEqual([]);
	});
});

describe('la ficha, una clave por contacto', () => {
	test('sin contacto, el «elegí a alguien» y ninguna ficha', () => {
		const view = detail(null);

		expect(view.text()).toContain('contact.pickOne');
		expect(view.find('header').exists()).toBe(false);
		expect(avisos).toEqual([]);
	});

	test('con contacto, la ficha y no el «elegí a alguien»', () => {
		const view = detail(ANA);

		expect(view.text()).not.toContain('contact.pickOne');
		expect(view.text()).toContain('Ana Pérez');
		expect(view.find('header').exists()).toBe(true);
	});

	test('el contenedor de la ficha no lleva overflow ni transform propios', () => {
		const card = detail(ANA).find('header').element.parentElement as HTMLElement;

		expect(card.className).not.toMatch(/overflow|transform|translate/);
		expect(card.className).toContain('min-w-0');
	});

	test('al elegir a otro contacto la ficha se remonta (otro nodo)', async () => {
		const view = detail(ANA);
		const antes = view.find('header').element.parentElement;

		await view.setProps({ contact: BRUNO });
		await flushPromises();

		const despues = view.find('header').element.parentElement;
		expect(despues).not.toBe(antes);
		expect(view.text()).toContain('Bruno Álvarez');
	});

	test('al refrescar al mismo contacto la ficha NO se remonta', async () => {
		const view = detail(ANA);
		const antes = view.find('header').element.parentElement;

		await view.setProps({ contact: { ...ANA, name: 'Ana P. Pérez' } });
		await flushPromises();

		expect(view.find('header').element.parentElement).toBe(antes);
		expect(view.text()).toContain('Ana P. Pérez');
	});

	test('un contacto sin url se distingue por el uid', async () => {
		const a = emptyContact({ name: 'Ana', uid: 'uid-a', url: '' });
		const b = emptyContact({ name: 'Beto', uid: 'uid-b', url: '' });
		const view = detail(a);
		const antes = view.find('header').element.parentElement;

		await view.setProps({ contact: b });
		await flushPromises();

		expect(view.find('header').element.parentElement).not.toBe(antes);
	});

	test('pasar de un contacto a ninguno y volver no deja dos fichas', async () => {
		const view = detail(ANA);
		await view.setProps({ contact: null });
		await flushPromises();
		await view.setProps({ contact: BRUNO });
		await flushPromises();

		expect(view.findAll('header')).toHaveLength(1);
		expect(view.text()).not.toContain('contact.pickOne');
		expect(avisos.filter((a) => /Transition|single element|multiple/i.test(a))).toEqual([]);
	});
});
