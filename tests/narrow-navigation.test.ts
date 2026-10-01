/**
 * Una columna por vez con la ventana angosta (`tools/narrow-layout.ts`).
 *
 * Qué columna se ve lo decide una consulta de contenedor sobre la fila de la
 * ventana, y happy-dom no calcula diseño: lo que se comprueba es la mitad que
 * sí es nuestra —a qué columna se llega con cada botón, que se pueda volver,
 * que el contacto elegido siga elegido y que el foco no se pierda— y que las
 * clases de la consulta sean las que se miden en el banco. Que con eso a 240 y
 * 360 se vea una columna entera, y desde 600 las tres como siempre, lo dicen
 * las capturas del PR.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { ActionButton, BarSearch, ListRow } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { effectScope } from 'vue';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import WindowAppLayout from '@/layouts/WindowAppLayout.vue';
import { emptyContact } from '@/tools/address-book';
import { useNarrowRow } from '@/composables/use-narrow-row';
import {
	NARROW_FULL_LINE,
	NARROW_ONLY,
	NARROW_ROW_WRAP,
	NARROW_WRAP,
	PANE_HIDDEN,
	PANE_SHOWN,
	paneClass,
} from '@/tools/narrow-layout';
import AddressBookView from '@/views/AddressBookView.vue';
import { answerCommand, olvidarTodo } from './dobles';

let mounted: VueWrapper | null = null;

const RealResizeObserver = globalThis.ResizeObserver;

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
	document.body.innerHTML = '';
	globalThis.ResizeObserver = RealResizeObserver;
});

/**
 * Un `ResizeObserver` que contesta con el ancho que se le diga, en el acto:
 * happy-dom no calcula diseño, así que el de verdad no avisa nunca.
 */
function rowWidth(width: number) {
	globalThis.ResizeObserver = class {
		constructor(private readonly callback: ResizeObserverCallback) {}
		observe(target: Element) {
			this.callback(
				[{ target, contentRect: { width } } as unknown as ResizeObserverEntry],
				this as unknown as ResizeObserver
			);
		}
		unobserve() {}
		disconnect() {}
	} as unknown as typeof ResizeObserver;
}

const ANA = emptyContact({ name: 'Ana Pérez', sort_key: 'Pérez, Ana', url: 'https://x/ana.vcf' });
const BRUNO = emptyContact({ name: 'Bruno Álvarez', sort_key: 'Álvarez, Bruno', url: 'https://x/bruno.vcf' });

async function view() {
	answerCommand('listar_cuentas', () => [{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false }]);
	answerCommand('contactos_de_la_cuenta', () => ({ contactos: [ANA, BRUNO], fallos: [] }));
	// Montado en el documento: el foco sólo se mueve entre elementos que están.
	const host = document.createElement('div');
	document.body.append(host);
	mounted = mount(AddressBookView, { attachTo: host });
	await flushPromises();
	return mounted;
}

/** La columna, por su `data-pane`. */
function pane(wrapper: VueWrapper, name: string) {
	return wrapper.find(`[data-pane="${name}"]`);
}

/** Cuál de las tres columnas es la que se ve con la ventana angosta. */
function shown(wrapper: VueWrapper): string[] {
	return ['accounts', 'list', 'detail'].filter((name) =>
		pane(wrapper, name).classes().includes('@max-[36rem]/row:w-full')
	);
}

/** El botón de ir o volver de una columna. */
function navButton(wrapper: VueWrapper, name: string) {
	return pane(wrapper, name).find('[data-nav]');
}

describe('una columna por vez', () => {
	test('se arranca por la lista, y las otras dos no se ven', async () => {
		const window = await view();

		expect(shown(window)).toEqual(['list']);
		expect(pane(window, 'accounts').classes()).toContain(PANE_HIDDEN);
		expect(pane(window, 'detail').classes()).toContain(PANE_HIDDEN);
	});

	test('elegir a alguien lleva a su ficha', async () => {
		const window = await view();
		await window.findAllComponents(ListRow)[1]?.trigger('click');
		await flushPromises();

		expect(shown(window)).toEqual(['detail']);
		expect(pane(window, 'detail').find('h1').text()).toBe('Ana Pérez');
	});

	test('de la ficha se vuelve a la lista, con el contacto todavía elegido', async () => {
		const window = await view();
		await window.findAllComponents(ListRow)[1]?.trigger('click');
		await flushPromises();
		await navButton(window, 'detail').trigger('click');
		await flushPromises();

		expect(shown(window)).toEqual(['list']);
		const rows = window.findAllComponents(ListRow);
		expect(rows.map((r) => r.props('selected'))).toEqual([false, true]);
		// Y la ficha sigue siendo la de Ana: volver a ella no la vacía.
		expect(pane(window, 'detail').find('h1').text()).toBe('Ana Pérez');
	});

	test('de la lista se llega a las cuentas, y de las cuentas se vuelve a la lista', async () => {
		const window = await view();
		await navButton(window, 'list').trigger('click');
		await flushPromises();
		expect(shown(window)).toEqual(['accounts']);
		expect(pane(window, 'accounts').text()).toContain('ana@nube.ejemplo.com');

		await navButton(window, 'accounts').trigger('click');
		await flushPromises();
		expect(shown(window)).toEqual(['list']);
	});

	test('el ida y vuelta completo no pierde al elegido', async () => {
		const window = await view();
		await window.findAllComponents(ListRow)[1]?.trigger('click');
		await flushPromises();
		await navButton(window, 'detail').trigger('click');
		await navButton(window, 'list').trigger('click');
		await navButton(window, 'accounts').trigger('click');
		await flushPromises();

		expect(shown(window)).toEqual(['list']);
		expect(window.findAllComponents(ListRow)[1]?.props('selected')).toBe(true);
	});

	test('buscar desde la ficha vuelve a la lista, sin sacarle el foco al campo', async () => {
		const window = await view();
		await window.findAllComponents(ListRow)[1]?.trigger('click');
		await flushPromises();
		const field = window.find('input');
		(field.element as HTMLInputElement).focus();
		await field.setValue('bru');
		await flushPromises();

		expect(shown(window)).toEqual(['list']);
		expect(document.activeElement).toBe(field.element);
	});

	test('el foco va al botón de la columna a la que se llega', async () => {
		// La que se deja se oculta, y un foco en algo oculto se pierde en el
		// documento: quien usa el teclado tendría que empezar de nuevo.
		const window = await view();
		await window.findAllComponents(ListRow)[1]?.trigger('click');
		await flushPromises();

		expect(document.activeElement).toBe(navButton(window, 'detail').element);

		await navButton(window, 'detail').trigger('click');
		await flushPromises();
		expect(document.activeElement).toBe(navButton(window, 'list').element);
	});
});

describe('los botones de ir y volver', () => {
	test('son `ActionButton` sin relleno, con el icono del tema y su nombre', async () => {
		const window = await view();
		const buttons = ['accounts', 'list', 'detail'].map((name) =>
			pane(window, name)
				.findAllComponents(ActionButton)
				.find((b: VueWrapper) => b.attributes('data-nav') !== undefined)
		);

		expect(buttons.map((b) => [b?.props('label'), b?.props('icon'), b?.props('iconType')])).toEqual([
			['nav.contacts', 'go-next', 'symbol'],
			['nav.accounts', 'go-previous', 'symbol'],
			['nav.contacts', 'go-previous', 'symbol'],
		]);
		expect(buttons.every((b) => b?.props('variant') === 'ghost')).toBe(true);
	});

	test('sólo existen con la ventana angosta', async () => {
		// Desde 36rem de fila se ocultan: la ventana de 600 se ve como siempre,
		// y la ficha no suma el renglón del botón.
		const window = await view();
		for (const name of ['accounts', 'list', 'detail']) {
			expect(navButton(window, name).element.parentElement?.className).toContain(NARROW_ONLY);
		}
	});
});

describe('lo decide la fila, no la pantalla', () => {
	test('la fila de la ventana es el contenedor `row`', () => {
		mounted = mount(WindowAppLayout, { slots: { default: '<p>x</p>' } });
		const row = mounted.find('p').element.parentElement;

		expect(row?.className).toContain('@container/row');
	});

	test('las clases son consultas de contenedor sobre `row`, ninguna de pantalla', () => {
		for (const classes of [PANE_SHOWN, PANE_HIDDEN, NARROW_ONLY, NARROW_WRAP, NARROW_ROW_WRAP, NARROW_FULL_LINE]) {
			for (const name of classes.split(' ')) {
				expect(name).toMatch(/^@(min|max)-\[36rem\]\/row:/);
			}
		}
	});

	test('cada columna recibe la de mostrarse o la de ocultarse', () => {
		expect(paneClass('list', 'list')).toBe(PANE_SHOWN);
		expect(paneClass('detail', 'list')).toBe(PANE_HIDDEN);
	});
});

describe('la barra con la ventana angosta', () => {
	test('con la fila ancha, el buscador abierto y la cuenta al lado, como siempre', async () => {
		rowWidth(1180);
		const window = await view();

		expect(window.findComponent(BarSearch).props('collapsed')).toBe(false);
		expect(window.find('[aria-live="polite"]').text()).toBe('lista.cuantosOther');
		expect(pane(window, 'list').find('[data-testid="count"]').exists()).toBe(false);
	});

	test('angosta, el buscador es la lupa y la cuenta baja a la lista', async () => {
		// A 240 el campo quedaba en «Bu» y «9 contactos» no se veía: ni cortado
		// ni desaparecido, la cuenta va al lado del botón de volver.
		rowWidth(300);
		const window = await view();

		expect(window.findComponent(BarSearch).props('collapsed')).toBe(true);
		const counts = window.findAll('[aria-live="polite"]');
		expect(counts).toHaveLength(1);
		expect(counts[0]?.attributes('data-testid')).toBe('count');
		expect(counts[0]?.text()).toBe('lista.cuantosOther');
	});

	test('angosta, los nombres de la lista se parten en vez de cortarse', async () => {
		rowWidth(300);
		const window = await view();

		expect(window.findAllComponents(ListRow).every((r) => r.props('truncate') === false)).toBe(true);
	});

	test('y la ficha pone los botones abajo del valor, entero', () => {
		mounted = mount(ContactDetail, {
			props: {
				contact: emptyContact({
					name: 'Ana Pérez',
					url: 'https://x/ana.vcf',
					emails: [{ label: 'work', value: 'ana.perez.con.un.correo.largo@estudiosur.example' }],
				}),
			},
		});
		const write = mounted.findAllComponents(ActionButton).find((b) => b.props('label') === 'contact.write');
		const row = write?.element.parentElement;
		const value = row?.querySelector('span');

		expect(row?.className).toContain(NARROW_ROW_WRAP);
		expect(value?.className).toContain(NARROW_WRAP);
		expect(value?.className).toContain(NARROW_FULL_LINE);
	});
});

describe('useNarrowRow', () => {
	test('angosta por debajo de 36rem de fila; sin ancho, queda lo que había', () => {
		const scope = effectScope();
		const result = scope.run(() => useNarrowRow(() => null));
		if (!result) throw new Error('sin resultado');
		const { narrow, measure } = result;
		expect(narrow.value).toBe(false);

		measure(575);
		expect(narrow.value).toBe(true);
		measure(576);
		expect(narrow.value).toBe(false);
		measure(500);
		measure(0);
		expect(narrow.value).toBe(true);
		scope.stop();
	});
});
