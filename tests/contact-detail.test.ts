/**
 * La ficha del contacto.
 *
 * Lo que se comprueba es lo que se rompe callado: que lo vacío no ocupe lugar,
 * que una web que no es web no tenga botón, y que **la foto externa no la pida
 * nunca la ventana** — se le pide al programa y lo que se dibuja es lo que él
 * contesta.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import I18n from '@vasakgroup/tauri-plugin-i18n';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { type Contact, emptyContact, type Relationship } from '@/tools/address-book';
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

describe('las relaciones', () => {
	function showRelated(related: Relationship[], allContacts: Contact[]) {
		wrapper = mount(ContactDetail, {
			props: { contact: emptyContact({ uid: 'yo', name: 'Ana Pérez', related }), allContacts },
		});
		return wrapper;
	}

	const luis = emptyContact({ uid: 'urn:uuid:luis', name: 'Luis Gómez' });

	test('aparece la sección con el tipo y el nombre de la ficha resuelta', () => {
		const view = showRelated([{ kind: 'spouse', name: '', target_uid: 'luis' }], [luis]);
		const text = view.find('[data-testid="relations"]').text();
		expect(text).toContain('form.relations.spouse');
		expect(text).toContain('Luis Gómez');
	});

	test('una relación resuelta tiene «abrir» y emite open con ese contacto', async () => {
		const view = showRelated([{ kind: 'spouse', name: '', target_uid: 'luis' }], [luis]);
		const open = view
			.findAll('[data-testid="relations"] button')
			.find((b) => b.text() === 'contact.open');
		expect(open).toBeDefined();
		await open?.trigger('click');
		expect(view.emitted('open')).toEqual([[luis]]);
	});

	test('una relación de sólo texto se ve sin botón «abrir»', () => {
		const view = showRelated([{ kind: 'friend', name: 'Mi vecina', target_uid: '' }], [luis]);
		const section = view.find('[data-testid="relations"]');
		expect(section.text()).toContain('Mi vecina');
		expect(section.findAll('button')).toHaveLength(0);
	});

	test('una referencia por uid que no resuelve y sin nombre no aparece', () => {
		const view = showRelated([{ kind: 'friend', name: '', target_uid: 'fantasma' }], [luis]);
		expect(view.find('[data-testid="relations"]').exists()).toBe(false);
	});

	// A declara que Luis es su pareja; en la ficha de Luis sale la inversa.
	const ana = emptyContact({
		uid: 'ana',
		name: 'Ana Pérez',
		related: [{ kind: 'spouse', name: '', target_uid: 'luis' }],
	});

	function showAsLuis(allContacts: Contact[], related: Relationship[] = []) {
		wrapper = mount(ContactDetail, {
			props: { contact: emptyContact({ ...luis, related }), allContacts },
		});
		return wrapper;
	}

	test('una relación inversa se muestra con el prefijo de «relationInverse»', () => {
		const text = showAsLuis([ana, luis]).find('[data-testid="relations"]').text();
		// t() sin cargar devuelve la clave cruda (sin {0}): interpolar la deja igual.
		expect(text).toContain('Ana Pérez');
		expect(text).toContain('contact.relationInverse');
		expect(text).not.toContain('form.relations.spouse ·');
	});

	test('la inversa resuelta tiene «abrir» y emite open con quien la declara', async () => {
		const view = showAsLuis([ana, luis]);
		const open = view
			.findAll('[data-testid="relations"] button')
			.find((b) => b.text() === 'contact.open');
		expect(open).toBeDefined();
		await open?.trigger('click');
		expect(view.emitted('open')).toEqual([[ana]]);
	});

	test('una relación mutua se muestra una sola vez', () => {
		const view = showAsLuis([ana, luis], [{ kind: 'spouse', name: '', target_uid: 'ana' }]);
		const text = view.find('[data-testid="relations"]').text();
		expect(text.split('Ana Pérez')).toHaveLength(2);
		expect(text).not.toContain('contact.relationInverse');
	});

	test('un tipo desconocido se muestra tal cual, sin traducir', () => {
		const view = showRelated([{ kind: 'tutor', name: 'Marta', target_uid: '' }], []);
		const text = view.find('[data-testid="relations"]').text();
		expect(text).toContain('tutor');
		expect(text).not.toContain('form.relations.tutor');
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

describe('el organigrama', () => {
	const ORG = 'Vasak Group';
	const jefa = emptyContact({ uid: 'jefa', name: 'Jefa Ruiz', organization: ORG });
	const asistente = emptyContact({ uid: 'asis', name: 'Asis Tente', organization: ORG });
	const ana = emptyContact({
		uid: 'ana',
		name: 'Ana Pérez',
		organization: ORG,
		related: [
			{ kind: 'manager', name: '', target_uid: 'jefa' },
			{ kind: 'assistant', name: '', target_uid: 'asis' },
		],
	});

	function showChart(contact: Contact, allContacts: Contact[]) {
		wrapper = mount(ContactDetail, { props: { contact, allContacts } });
		return wrapper;
	}

	test('aparece con los nombres y marca al asistente', () => {
		const section = showChart(ana, [jefa, ana, asistente]).find('[data-testid="org-chart"]');
		expect(section.exists()).toBe(true);
		expect(section.text()).toContain('contact.orgChart');
		expect(section.text()).toContain('Jefa Ruiz');
		expect(section.text()).toContain('Asis Tente');
		expect(section.text()).toContain('· form.relations.assistant');
	});

	test('el contacto actual no tiene «abrir» y los demás sí', () => {
		const section = showChart(ana, [jefa, ana, asistente]).find('[data-testid="org-chart"]');
		expect(section.findAll('button').filter((b) => b.text() === 'contact.open')).toHaveLength(2);
	});

	test('«abrir» emite open con ese contacto', async () => {
		const view = showChart(ana, [jefa, ana, asistente]);
		const open = view.findAll('[data-testid="org-chart"] button')[0];
		await open.trigger('click');
		expect(view.emitted('open')).toEqual([[jefa]]);
	});

	test('sin organigrama la sección no se muestra', () => {
		const solo = emptyContact({ uid: 'solo', name: 'Solo', organization: ORG });
		expect(showChart(solo, [solo]).find('[data-testid="org-chart"]').exists()).toBe(false);
	});

	test('el árbol lleva roles y la profundidad en aria-level', () => {
		const abuela = emptyContact({ uid: 'abu', name: 'Abu Ela', organization: ORG });
		const jefaConJefa = { ...jefa, related: [{ kind: 'manager' as const, name: '', target_uid: 'abu' }] };
		const view = showChart(ana, [abuela, jefaConJefa, ana]);
		const section = view.find('[data-testid="org-chart"]');
		expect(section.find('[role="tree"]').exists()).toBe(true);
		const levels = section
			.findAll('[role="treeitem"]')
			.map((n) => [n.text().replace(/\s+/g, ' ').trim(), n.attributes('aria-level')]);
		expect(levels.map(([, l]) => l)).toEqual(['1', '2', '3']);
		expect(levels[0][0]).toContain('Abu Ela');
		expect(levels[1][0]).toContain('Jefa Ruiz');
		expect(levels[2][0]).toContain('Ana Pérez');
	});

	test('cada «abrir» dice a quién abre', async () => {
		await I18n.getInstance().load();
		const section = showChart(ana, [jefa, ana, asistente]).find('[data-testid="org-chart"]');
		const labels = section.findAll('button').map((b) => b.attributes('aria-label'));
		expect(labels).toEqual(['Abrir a Jefa Ruiz', 'Abrir a Asis Tente']);
	});
});
