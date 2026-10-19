/**
 * El historial de correo de la ficha (vasak-contacts#11, tramo correo).
 *
 * Lo que se rompe callado: que apagado consulte igual (una fuga de privacidad
 * que no se ve), que un «no» se pinte como error roto, y que dos direcciones del
 * mismo contacto repitan mensajes.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { emptyContact } from '@/tools/address-book';
import {
	collectHistory,
	historyAddresses,
	loadMailHistoryEnabled,
	type MailInteraction,
	mergeHistory,
	saveMailHistoryEnabled,
} from '@/tools/mail-history';
import { STORE_PERMISSION_DENIED } from '@/tools/store-permission';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

const mail = (uid: number, date_ts: number, subject = `s${uid}`): MailInteraction => ({
	uid,
	date_ts,
	subject,
	from_addr: 'ana@x.com',
});

describe('qué direcciones se preguntan', () => {
	test('se recortan, no se repiten y se descartan las que no son direcciones', () => {
		const emails = [' Ana@x.com ', 'ana@X.COM', 'sin-arroba', ''].map((value) => ({
			label: '',
			value,
		}));
		expect(historyAddresses(emails)).toEqual(['Ana@x.com']);
	});
});

describe('juntar lo de varias direcciones', () => {
	test('más nuevos primero, sin repetir y acotado', () => {
		const merged = mergeHistory([[mail(1, 10), mail(2, 30)], [mail(2, 30), mail(3, 20)]], 2);
		expect(merged.map((m) => m.uid)).toEqual([2, 3]);
	});

	test('un «no» corta y el resto de fallos es «no disponible»', async () => {
		const denied = await collectHistory(['a@x', 'b@x'], async () => {
			throw STORE_PERMISSION_DENIED;
		});
		expect(denied.state).toBe('denied');
		const broken = await collectHistory(['a@x', 'b@x'], async () => {
			throw 'otra cosa';
		});
		expect(broken.state).toBe('unavailable');
	});

	test('si falla sólo una dirección se muestra lo que salió', async () => {
		const out = await collectHistory(['a@x', 'b@x'], async (a) => {
			if (a === 'a@x') throw 'otra cosa';
			return [mail(1, 5)];
		});
		expect(out.state).toBe('ready');
		expect(out.items).toHaveLength(1);
	});
});

describe('la preferencia', () => {
	afterEach(() => localStorage.clear());

	test('apagada por omisión, y se recuerda al encenderla', () => {
		expect(loadMailHistoryEnabled()).toBe(false);
		saveMailHistoryEnabled(true);
		expect(loadMailHistoryEnabled()).toBe(true);
		saveMailHistoryEnabled(false);
		expect(loadMailHistoryEnabled()).toBe(false);
	});
});

describe('la sección de la ficha', () => {
	let wrapper: VueWrapper | null = null;
	const contact = () =>
		emptyContact({
			name: 'Ana',
			account_id: 'acc1',
			emails: [{ label: '', value: 'ana@x.com' }],
		});
	const asked = () => invokedCommands.filter((c) => c.command === 'mail_history');

	afterEach(() => {
		wrapper?.unmount();
		wrapper = null;
		localStorage.clear();
		olvidarTodo();
	});

	test('apagada no consulta nada y ofrece encenderla', async () => {
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(asked()).toHaveLength(0);
		expect(wrapper.find('[data-testid="mail-history-off"]').exists()).toBe(true);
	});

	test('encendida consulta por la cuenta y la dirección, y lista los mensajes', async () => {
		saveMailHistoryEnabled(true);
		answerCommand('mail_history', () => [mail(7, 1_700_000_000, 'Hola')]);
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(asked()[0]?.args).toEqual({ accountId: 'acc1', address: 'ana@x.com', limit: 10 });
		expect(wrapper.find('[data-testid="mail-history-list"]').text()).toContain('Hola');
	});

	test('un «no» muestra el estado sin permiso con el botón de pedir de nuevo', async () => {
		saveMailHistoryEnabled(true);
		answerCommand('mail_history', () => {
			throw STORE_PERMISSION_DENIED;
		});
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(wrapper.find('[data-testid="mail-history-denied"]').exists()).toBe(true);
	});
});
