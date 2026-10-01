/**
 * La ficha del contacto.
 *
 * Lo que se comprueba es lo que se rompe callado: que lo vacío no ocupe lugar,
 * que una web que no es web no tenga botón, y que **la foto externa no la pida
 * nunca la ventana** — se le pide al programa y lo que se dibuja es lo que él
 * contesta.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { type Contact, emptyContact } from '@/tools/address-book';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

const PNG = 'data:image/png;base64,iVBORw0KGgo=';

let wrapper: VueWrapper | null = null;

function show(contact: Partial<Contact>) {
	wrapper = mount(ContactDetail, {
		props: { contact: emptyContact({ name: 'Ana Pérez', url: 'https://x/ana.vcf', ...contact }) },
	});
	return wrapper;
}

afterEach(() => {
	wrapper?.unmount();
	wrapper = null;
	olvidarTodo();
});

/** Lo que se le pidió abrir al sistema. */
function opened(): unknown[] {
	return invokedCommands
		.filter((c) => c.command === 'plugin:shell|open')
		.map((c) => c.args?.path);
}

describe('lo que está vacío no se muestra', () => {
	test('un contacto con sólo el nombre es una ficha corta', () => {
		const view = show({});
		for (const section of ['addresses', 'dates', 'websites', 'social', 'more', 'photo-note']) {
			expect(view.find(`[data-testid="${section}"]`).exists()).toBe(false);
		}
		expect(view.find('[data-testid="nickname"]').exists()).toBe(false);
		expect(view.find('[data-testid="position"]').exists()).toBe(false);
	});
});

describe('los campos nuevos', () => {
	test('el apodo, el cargo, la función y la organización', () => {
		const view = show({
			nickname: 'Anita',
			title: 'Jefa de soporte',
			role: 'Programadora',
			organization: 'Vasak Group',
		});
		expect(view.find('[data-testid="nickname"]').text()).toContain('Anita');
		const position = view.find('[data-testid="position"]').text();
		expect(position).toContain('Jefa de soporte · Programadora');
		expect(position).toContain('Vasak Group');
	});

	test('la dirección en renglones, sin los vacíos', () => {
		const view = show({
			addresses: [
				{
					label: 'home',
					po_box: '',
					extended: '',
					street: 'Av. Siempreviva 742',
					locality: 'Springfield',
					region: '',
					postal_code: 'B1636',
					country: 'Argentina',
				},
			],
		});
		const lines = view.findAll('[data-testid="addresses"] address span').map((s) => s.text());
		expect(lines).toEqual(['Av. Siempreviva 742', 'Springfield', 'B1636', 'Argentina']);
	});

	test('el cumpleaños sin año no inventa uno', () => {
		const view = show({
			birthday: { year: null, month: 4, day: 15, text: '' },
			anniversary: { year: 2010, month: 6, day: 12, text: '' },
		});
		const dates = view.find('[data-testid="dates"]').text();
		expect(dates).toContain('contact.birthday');
		expect(dates).toContain('contact.anniversary');
		expect(dates).toContain('2010');
		// El único año que aparece es el del aniversario.
		expect(dates.match(/\d{4}/g)).toEqual(['2010']);
	});

	test('las redes con el nombre del servicio', () => {
		const view = show({
			social: [
				{ service: 'twitter', label: '', handle: 'anaperez', url: 'https://twitter.com/anaperez' },
				{ service: 'xmpp', label: 'home', handle: 'ana@jabber.org', url: '' },
			],
		});
		const text = view.find('[data-testid="social"]').text();
		expect(text).toContain('Twitter · anaperez');
		expect(text).toContain('XMPP · ana@jabber.org');
	});

	test('los datos que se consultan poco van plegados', () => {
		const view = show({
			languages: ['es-AR'],
			time_zone: '-0300',
			custom_fields: [{ name: 'X-PHONETIC-FIRST-NAME', label: '', value: 'A-na' }],
		});
		const more = view.find('[data-testid="more"]');
		// Plegado de verdad: la cabecera dice que está cerrado, y lo de adentro
		// está pero no se ve hasta abrirlo.
		expect(more.find('button').attributes('aria-expanded')).toBe('false');
		expect(more.text()).toContain('-0300');
		expect(more.text()).toContain('Phonetic first name');
		expect(more.text()).toContain('A-na');
	});
});

describe('las webs', () => {
	test('una web de verdad se abre con el sistema', async () => {
		const view = show({ websites: [{ label: 'homepage', value: 'ana.ejemplo.com' }] });
		const open = view
			.findAll('[data-testid="websites"] button')
			.find((b) => b.text() === 'contact.open');
		expect(open).toBeDefined();
		await open?.trigger('click');
		await flushPromises();
		expect(opened()).toEqual(['https://ana.ejemplo.com/']);
	});

	test('una «web» que no es web no tiene botón para abrirla', () => {
		const view = show({ websites: [{ label: '', value: 'javascript:alert(1)' }] });
		const buttons = view.findAll('[data-testid="websites"] button').map((b) => b.text());
		expect(buttons).not.toContain('contact.open');
		// Pero se ve y se puede copiar: es un dato de la tarjeta.
		expect(view.find('[data-testid="websites"]').text()).toContain('javascript:alert(1)');
	});
});

describe('la foto', () => {
	test('la que vino en la tarjeta se muestra', () => {
		const view = show({ photo: PNG });
		expect(view.find('[data-testid="contact-photo"] img').attributes('src')).toBe(PNG);
		expect(invokedCommands.filter((c) => c.command === 'contact_photo')).toHaveLength(0);
	});

	test('sin foto, las iniciales', () => {
		const view = show({});
		expect(view.find('[data-testid="contact-photo"] img').exists()).toBe(false);
		expect(view.find('[data-testid="contact-photo"]').text()).toBe('AP');
	});

	test('la externa se le pide al programa, y nunca va al src', async () => {
		answerCommand('contact_photo', () => PNG);
		const remote = 'https://lh3.googleusercontent.com/contacts/uno';
		const view = show({ photo_url: remote });

		// Antes de que conteste, las iniciales: la dirección no va a ningún
		// lado de la ventana.
		expect(view.html()).not.toContain('googleusercontent');
		await flushPromises();

		expect(invokedCommands.filter((c) => c.command === 'contact_photo')).toEqual([
			{ command: 'contact_photo', args: { url: remote } },
		]);
		expect(view.find('[data-testid="contact-photo"] img').attributes('src')).toBe(PNG);
		expect(view.html()).not.toContain('googleusercontent');
	});

	test('si el programa no la consigue, quedan las iniciales', async () => {
		answerCommand('contact_photo', () => null);
		const view = show({ photo_url: 'https://lh3.googleusercontent.com/contacts/dos' });
		await flushPromises();
		expect(view.find('[data-testid="contact-photo"] img').exists()).toBe(false);
		expect(view.find('[data-testid="contact-photo"]').text()).toBe('AP');
	});

	test('si el motor no puede dibujarla, vuelven las iniciales', async () => {
		const view = show({ photo: PNG });
		await view.find('[data-testid="contact-photo"] img').trigger('error');
		expect(view.find('[data-testid="contact-photo"] img').exists()).toBe(false);
		expect(view.find('[data-testid="contact-photo"]').text()).toBe('AP');
	});

	test('se dice por qué no hay foto cuando vale la pena', () => {
		expect(show({ photo_skipped: 'too_large' }).find('[data-testid="photo-note"]').text()).toBe(
			'contact.photoTooLarge'
		);
		wrapper?.unmount();
		expect(show({ photo_skipped: 'insecure' }).find('[data-testid="photo-note"]').text()).toBe(
			'contact.photoInsecure'
		);
		wrapper?.unmount();
		// Una que no es imagen no merece explicación: se ven las iniciales.
		expect(show({ photo_skipped: 'unsupported' }).find('[data-testid="photo-note"]').exists()).toBe(
			false
		);
	});
});

describe('los textos de la ficha', () => {
	/** Las claves de un catálogo, aplanadas como las aplana el plugin. */
	async function keysOf(language: string): Promise<Set<string>> {
		const text = await Bun.file(`${import.meta.dir}/../src-tauri/locales/${language}.yml`).text();
		const out = new Set<string>();
		const walk = (value: unknown, prefix: string) => {
			if (value && typeof value === 'object') {
				for (const [k, v] of Object.entries(value)) walk(v, prefix ? `${prefix}.${k}` : k);
			} else {
				out.add(prefix);
			}
		};
		walk(Bun.YAML.parse(text), '');
		return out;
	}

	test('todas las claves que usa la ficha existen en los dos idiomas', async () => {
		const sources = await Promise.all(
			[
				'../src/components/contacts/ContactDetail.vue',
				'../src/components/contacts/ContactPhoto.vue',
				'../src/tools/contact-fields.ts',
			].map((p) => Bun.file(`${import.meta.dir}/${p}`).text())
		);
		const used = new Set<string>();
		for (const source of sources) {
			// `t(` suelto: sin el borde, `emit('back')` contaba como la clave
			// «back».
			for (const match of source.matchAll(/(?<![\w$])t\('([\w.]+)'\)/g)) used.add(match[1]);
			// Las que se arman con un nombre: `contact.kinds.${kind}` y las
			// etiquetas de `labelKey`.
		}
		for (const kind of ['org', 'group', 'location']) used.add(`contact.kinds.${kind}`);
		const labelsSource = sources[2];
		const labels = labelsSource
			.slice(labelsSource.indexOf('const KNOWN_LABELS'), labelsSource.indexOf(']);'))
			.matchAll(/'(\w+)'/g);
		for (const match of labels) used.add(`contact.labels.${match[1]}`);

		expect(used.size).toBeGreaterThan(30);
		for (const language of ['es', 'en']) {
			const keys = await keysOf(language);
			const missing = [...used].filter((k) => !keys.has(k));
			expect(missing).toEqual([]);
		}
	});
});
