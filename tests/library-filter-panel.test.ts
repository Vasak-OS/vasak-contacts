/**
 * El panel de cuentas dibuja el elegir-libretas y el filtro de categorías con
 * los componentes de la librería, y avisa hacia afuera (vasak-contacts#4).
 *
 * Lo que decide qué se ve está probado en `library-filter.test.ts` y en
 * `use-agenda.test.ts`; acá se mira que el panel use `Checkbox` y `Chip` y que
 * sus eventos lleguen.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { Checkbox, Chip } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import AccountsPanel from '@/components/contacts/AccountsPanel.vue';
import { bookKey } from '@/tools/library-filter';
import { olvidarTodo } from './dobles';

let mounted: VueWrapper | null = null;

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
});

const BOOKS = [
	{ accountId: 'a1', accountName: 'ana@nube', id: 'personal', name: 'Personal', key: bookKey('a1', 'personal') },
	{ accountId: 'a1', accountName: 'ana@nube', id: 'trabajo', name: 'Trabajo', key: bookKey('a1', 'trabajo') },
];

function panel(props: Record<string, unknown> = {}) {
	mounted = mount(AccountsPanel, {
		props: {
			accounts: [{ id: 'a1', nombre: 'ana@nube', necesita_reconectarse: false }],
			notices: [],
			...props,
		},
	});
	return mounted;
}

describe('elegir qué libretas se ven', () => {
	test('una casilla de la librería por libreta, marcada = se ve', () => {
		const view = panel({ addressBooks: BOOKS, hiddenBooks: new Set([bookKey('a1', 'trabajo')]) });
		const boxes = view.findAllComponents(Checkbox);

		expect(boxes.map((b) => b.props('label'))).toEqual(['Personal', 'Trabajo']);
		// La oculta va desmarcada; la que se ve, marcada.
		expect(boxes.map((b) => b.props('modelValue'))).toEqual([true, false]);
	});

	test('con una sola libreta no hay nada que elegir y no se dibuja', () => {
		const view = panel({ addressBooks: [BOOKS[0]] });
		expect(view.find('[data-testid="books"]').exists()).toBe(false);
		expect(view.findAllComponents(Checkbox)).toHaveLength(0);
	});

	test('tocar una casilla avisa cuál libreta, por su clave', async () => {
		const view = panel({ addressBooks: BOOKS, hiddenBooks: new Set() });
		await view.findAllComponents(Checkbox)[1]?.vm.$emit('change', false);

		expect(view.emitted('toggle-book')?.[0]).toEqual([bookKey('a1', 'trabajo')]);
	});
});

describe('filtrar por categoría', () => {
	test('una pastilla de la librería por categoría, la elegida con tilde y aria-pressed', () => {
		const view = panel({ categories: ['Amigos', 'Trabajo'], selectedCategories: new Set(['Trabajo']) });
		const chips = view.findAllComponents(Chip);

		expect(chips.map((c) => c.props('label'))).toEqual(['Amigos', 'Trabajo']);
		expect(chips.every((c) => c.props('interactive'))).toBe(true);
		// La elegida lleva el tilde del tema; la otra no.
		expect(chips[0]?.props('icon')).toBe('');
		expect(chips[1]?.props('icon')).toBe('object-select');
		expect(chips[1]?.attributes('aria-pressed')).toBe('true');
		expect(chips[0]?.attributes('aria-pressed')).toBe('false');
	});

	test('sin categorías no se dibuja la sección', () => {
		expect(panel({ categories: [] }).find('[data-testid="categories"]').exists()).toBe(false);
	});

	test('tocar una pastilla avisa cuál categoría', async () => {
		const view = panel({ categories: ['Amigos'], selectedCategories: new Set() });
		await view.findComponent(Chip).trigger('click');

		expect(view.emitted('toggle-category')?.[0]).toEqual(['Amigos']);
	});

	test('el botón de limpiar sólo aparece con algo elegido, y avisa', async () => {
		expect(
			panel({ categories: ['Amigos'], selectedCategories: new Set() }).find('[data-testid="categories"] button[aria-label]').exists()
		).toBe(false);

		const view = panel({ categories: ['Amigos'], selectedCategories: new Set(['Amigos']) });
		const clear = view
			.findAll('[data-testid="categories"] button')
			.find((b) => b.attributes('aria-label') === 'filtros.limpiarCategorias');
		expect(clear).toBeDefined();
		await clear?.trigger('click');
		expect(view.emitted('clear-categories')).toHaveLength(1);
	});
});
