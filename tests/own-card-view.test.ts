/**
 * «Este soy yo» y «Mi código QR» en la ventana (vasak-contacts#10).
 *
 * El cableado: la ficha avisa qué se tocó, la vista marca la tarjeta propia y
 * recuerda la marca, y «Mi QR» abre el código de la tarjeta marcada, o propone
 * la que dice el servidor para que la persona la confirme. La marca es de la
 * persona: lo que dice el servidor nunca se marca solo.
 */

import { afterEach, beforeEach, describe, expect, spyOn, test } from 'bun:test';
import { ActionButton, Badge, ListRow } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import OwnCardDialog from '@/components/contacts/OwnCardDialog.vue';
import QrDialog from '@/components/contacts/QrDialog.vue';
import { emptyContact } from '@/tools/address-book';
import { loadOwnCardKey, saveOwnCardKey } from '@/tools/own-card';
import AddressBookView from '@/views/AddressBookView.vue';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

const ANA = emptyContact({
	name: 'Ana Pérez',
	sort_key: 'Pérez, Ana',
	url: 'store:a1:/dav/book/ana.vcf',
	phones: [{ label: '', value: '111' }],
});
const BRUNO = emptyContact({
	name: 'Bruno Álvarez',
	sort_key: 'Álvarez, Bruno',
	url: 'store:a1:/dav/book/bruno.vcf',
});

let mounted: VueWrapper | null = null;

beforeEach(() => {
	globalThis.localStorage?.clear();
});
afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
	globalThis.localStorage?.clear();
	document.body.innerHTML = '';
});

describe('la ficha', () => {
	function detail(isOwn: boolean) {
		mounted = mount(ContactDetail, { props: { contact: ANA, isOwn } });
		return mounted;
	}

	const byTestId = (wrapper: VueWrapper, id: string) =>
		wrapper.findAllComponents(ActionButton).find((b) => b.attributes('data-testid') === id);

	test('«este soy yo» y «mostrar QR» avisan con el contacto de la ficha', async () => {
		const wrapper = detail(false);

		await byTestId(wrapper, 'toggle-own')?.trigger('click');
		await byTestId(wrapper, 'show-qr')?.trigger('click');

		expect(wrapper.emitted('toggleOwn')).toEqual([[ANA]]);
		expect(wrapper.emitted('qr')).toEqual([[ANA]]);
	});

	test('en la tarjeta propia el botón está apretado y hay una insignia; en las demás, no', () => {
		const own = detail(true);
		expect(byTestId(own, 'toggle-own')?.props('pressed')).toBe(true);
		expect(own.findAllComponents(Badge).some((b) => b.props('label') === 'qr.isMe')).toBe(true);
		own.unmount();

		const other = detail(false);
		expect(byTestId(other, 'toggle-own')?.props('pressed')).toBe(false);
		expect(other.findAllComponents(Badge).some((b) => b.props('label') === 'qr.isMe')).toBe(false);
	});
});

describe('la ventana', () => {
	async function view(contacts = [ANA, BRUNO]) {
		answerCommand('listar_cuentas', () => [
			{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
		]);
		answerCommand('contactos_de_la_cuenta', () => ({ libretas: [], contactos: contacts, fallos: [] }));
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();
		return mounted;
	}

	const myQr = (wrapper: VueWrapper) =>
		wrapper.findAllComponents(ActionButton).find((b) => b.attributes('data-testid') === 'my-qr');

	test('«este soy yo» en la ficha elegida marca la tarjeta, la recuerda y la desmarca al repetir', async () => {
		const wrapper = await view();
		// El orden de la lista es el alfabético: Álvarez, Pérez.
		await wrapper.findAllComponents(ListRow)[1]?.trigger('click');
		const toggle = () =>
			wrapper
				.findComponent(ContactDetail)
				.findAllComponents(ActionButton)
				.find((b) => b.attributes('data-testid') === 'toggle-own');
		const badge = () => wrapper.findComponent(ContactDetail).find('[data-testid="own-badge"]');
		expect(wrapper.findComponent(ContactDetail).props('contact')?.name).toBe('Ana Pérez');
		expect(badge().exists()).toBe(false);

		await toggle()?.trigger('click');

		expect(loadOwnCardKey()).toBe('store:a1:/dav/book/ana.vcf');
		expect(badge().exists()).toBe(true);
		expect(toggle()?.props('pressed')).toBe(true);

		await toggle()?.trigger('click');

		expect(loadOwnCardKey()).toBe('');
		expect(badge().exists()).toBe(false);
	});

	test('«mostrar QR» en la ficha abre el código de ese contacto', async () => {
		const wrapper = await view();
		await wrapper.findAllComponents(ListRow)[1]?.trigger('click');

		await wrapper
			.findComponent(ContactDetail)
			.findAllComponents(ActionButton)
			.find((b) => b.attributes('data-testid') === 'show-qr')
			?.trigger('click');

		expect(wrapper.findComponent(QrDialog).props('open')).toBe(true);
		expect(wrapper.findComponent(QrDialog).props('contact')?.name).toBe('Ana Pérez');
	});

	test('«Mi QR» no está si no hay agenda que mirar', async () => {
		const wrapper = await view([]);

		expect(myQr(wrapper)).toBeUndefined();
	});

	test('con la tarjeta propia marcada, «Mi QR» abre su código sin preguntarle al servidor', async () => {
		saveOwnCardKey('store:a1:/dav/book/ana.vcf');
		const wrapper = await view();

		await myQr(wrapper)?.trigger('click');
		await flushPromises();

		const qr = wrapper.findComponent(QrDialog);
		expect(qr.props('open')).toBe(true);
		expect(qr.props('contact')?.name).toBe('Ana Pérez');
		expect(invokedCommands.map((c) => c.command)).not.toContain('own_card_address');
		expect(wrapper.findComponent(OwnCardDialog).props('open')).toBe(false);
	});

	test('sin marca, propone la que dice el servidor y no la marca hasta que la persona confirma', async () => {
		answerCommand('own_card_address', () => '/dav/book/bruno.vcf');
		const wrapper = await view();

		await myQr(wrapper)?.trigger('click');
		await flushPromises();

		const dialog = wrapper.findComponent(OwnCardDialog);
		expect(dialog.props('open')).toBe(true);
		expect(dialog.props('proposal')?.name).toBe('Bruno Álvarez');
		expect(wrapper.findComponent(QrDialog).props('open')).toBe(false);
		expect(loadOwnCardKey()).toBe('');

		dialog.vm.$emit('confirm');
		await flushPromises();

		expect(loadOwnCardKey()).toBe('store:a1:/dav/book/bruno.vcf');
		expect(dialog.props('open')).toBe(false);
		expect(wrapper.findComponent(QrDialog).props('open')).toBe(true);
		expect(wrapper.findComponent(QrDialog).props('contact')?.name).toBe('Bruno Álvarez');
	});

	test('si el servidor no lo dice o falla, el diálogo lo explica y no marca nada', async () => {
		const log = spyOn(console, 'error').mockImplementation(() => {});
		answerCommand('own_card_address', () => {
			throw 'sin permiso';
		});
		const wrapper = await view();

		await myQr(wrapper)?.trigger('click');
		await flushPromises();

		const dialog = wrapper.findComponent(OwnCardDialog);
		expect(dialog.props('open')).toBe(true);
		expect(dialog.props('proposal')).toBeNull();
		expect(dialog.props('loading')).toBe(false);
		expect(wrapper.findComponent(QrDialog).props('open')).toBe(false);
		expect(loadOwnCardKey()).toBe('');
		log.mockRestore();
	});
});
