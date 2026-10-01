/**
 * El largo de la ficha no cambió al pasar a los componentes de la 2.2.0.
 *
 * Medido en el banco de Vite con Chrome sin pantalla, con el contacto completo
 * del banco (Ana Pérez: dos correos, un teléfono, una dirección, cumpleaños,
 * web, nota, y en «Más datos» los idiomas y la zona horaria), la ficha medía
 * antes y mide después, en píxeles:
 *
 * | ventana | «Más datos» cerrado | abierto            |
 * |---------|---------------------|--------------------|
 * | 1200    | 705 → 705           | 757 → 757          |
 * | 600     | 805 → 805           | 877 → 877          |
 *
 * A 600 la ficha mide unos 230 px, por debajo de los 20rem en los que la
 * `PropertyList` pone el nombre arriba del valor: eso daba 12 px más. Con la
 * ventana ancha se le fijan las dos columnas del `dl` de antes.
 *
 * happy-dom no calcula diseño, así que lo que queda fijo acá son las piezas
 * de las que sale esa cuenta. Si una cambia —la librería agranda la cabecera
 * del `Disclosure`, alguien saca un margen—, esta prueba se cae y hay que
 * volver a medir en el banco antes de tocar el número.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { Avatar, Disclosure, PropertyList } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { type Contact, emptyContact } from '@/tools/address-book';
import { olvidarTodo } from './dobles';

let mounted: VueWrapper | null = null;

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
});

/** La Ana del banco, con lo que hace al largo de la ficha. */
const ANA: Partial<Contact> = {
	name: 'Ana Pérez',
	nickname: 'Anita',
	title: 'Diseñadora',
	organization: 'Estudio Sur',
	categories: ['Trabajo', 'Amigos'],
	languages: ['es', 'en'],
	time_zone: 'America/Argentina/Cordoba',
};

function detail() {
	mounted = mount(ContactDetail, {
		props: { contact: emptyContact({ url: 'https://x/ana.vcf', ...ANA }) },
	});
	return mounted;
}

/** Los píxeles de una clase de espaciado de Tailwind (`-my-1.5` → -6). */
function px(classes: string[], prefix: string): number | undefined {
	for (const name of classes) {
		const negative = name.startsWith('-');
		const bare = negative ? name.slice(1) : name;
		if (bare.startsWith(`${prefix}-`)) {
			const step = Number(bare.slice(prefix.length + 1));
			if (Number.isFinite(step)) return (negative ? -1 : 1) * step * 4;
		}
	}
	return undefined;
}

describe('la ficha mide lo mismo', () => {
	test('la foto es la caja de 64 px de siempre', () => {
		const avatar = detail().findComponent(Avatar);

		expect(avatar.props('size')).toBe('xl');
		expect(avatar.classes()).toContain('size-16');
	});

	test('«Más datos» cerrado ocupa los 20 px del `summary` de antes', () => {
		const more = detail().findComponent(Disclosure);
		const header = more.find('button');

		// La cabecera de la librería mide 32 (el mínimo para tocarla)...
		expect(header.classes()).toContain('min-h-8');
		// ...y los márgenes le devuelven 12 a la ficha.
		const outer = more.classes();
		expect(32 + (px(outer, 'my') ?? 0) * 2).toBe(20);
	});

	test('abierto, el margen de abajo se come el relleno y el hueco que suma la librería', () => {
		const more = detail().findComponent(Disclosure);
		const outer = more.classes();
		const region = more.find('[id$="-region"]');
		const dl = more.findComponent(PropertyList).find('dl');

		// Lo que la región y la lista suman respecto del `mt-2` y el `gap-y-1`
		// de antes: 4 de relleno (pt-1 + pb-2 = 12 contra 8) y 4 entre filas.
		const padding = (px(region.classes(), 'pt') ?? 0) + (px(region.classes(), 'pb') ?? 0);
		const gap = px(dl.classes(), 'gap-y') ?? 0;
		expect(padding - 8 + (gap - 4)).toBe(8);

		// Y abierto, el margen de abajo pasa de -6 a -14: esos 8.
		expect(outer).toContain('data-[open=true]:-mb-3.5');
		expect(-14 - (px(outer, 'my') ?? 0)).toBe(-8);
	});

	test('con la ventana ancha, «Más datos» va en dos columnas como el `dl` de antes', () => {
		const list = detail().findComponent(PropertyList);

		expect(list.classes()).toContain(
			'@min-[36rem]/row:[&_dl]:grid-cols-[minmax(0,max-content)_minmax(0,1fr)]'
		);
		expect(list.classes()).toContain('@min-[36rem]/row:[&_dl>div]:col-span-2');
	});

	test('la cabecera conserva su relleno y su distribución', () => {
		const header = detail().find('header');

		expect(header.classes()).toEqual(
			expect.arrayContaining(['p-4', 'gap-4', 'flex-col', '@min-[12rem]:flex-row', '@min-[12rem]:items-center'])
		);
	});
});
