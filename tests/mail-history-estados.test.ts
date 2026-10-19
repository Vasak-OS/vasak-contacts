/**
 * El historial de correo de la ficha: lo que `mail-history.test.ts` deja sin
 * fijar (vasak-contacts#11, tramo correo).
 *
 * Cada prueba tiene su sabotaje mínimo: quitar el código que protege lo que
 * dice el título la pone en rojo. Lo que se vigila es lo que falla callado: un
 * contacto repetido, una consulta con la sección apagada, un «no» que se repite
 * en cada ficha, un error crudo en pantalla y, sobre todo, texto ajeno (asunto y
 * remitente, que elige quien escribe) interpretado como HTML.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { type Contact, emptyContact } from '@/tools/address-book';
import {
	collectHistory,
	historyAddresses,
	loadMailHistoryEnabled,
	MAIL_HISTORY_LIMIT,
	MAX_HISTORY_ADDRESSES,
	type MailInteraction,
	mergeHistory,
	saveMailHistoryEnabled,
} from '@/tools/mail-history';
import { STORE_PERMISSION_DENIED } from '@/tools/store-permission';
import { answerCommand, CATALOGO, invokedCommands, olvidarTodo } from './dobles';

const mail = (uid: number, date_ts: number, subject = `s${uid}`, from_addr = 'ana@x.com') =>
	({ uid, date_ts, subject, from_addr }) satisfies MailInteraction;

describe('juntar las direcciones del contacto', () => {
	test('un mismo mensaje que coincide por dos direcciones aparece una sola vez', () => {
		const merged = mergeHistory([[mail(1, 10)], [mail(1, 10)]], 10);
		expect(merged).toHaveLength(1);
	});

	test('van del más nuevo al más viejo, y el que no trae fecha al final', () => {
		const merged = mergeHistory([[mail(1, 10), mail(2, 0)], [mail(3, 30), mail(4, 20)]], 10);
		expect(merged.map((m) => m.uid)).toEqual([3, 4, 1, 2]);
	});

	test('la ficha muestra diez como mucho', async () => {
		const quince = Array.from({ length: 15 }, (_, i) => mail(i + 1, i + 1));
		const out = await collectHistory(['a@x'], async () => quince);
		expect(MAIL_HISTORY_LIMIT).toBe(10);
		expect(out.items).toHaveLength(10);
		expect(out.items[0]?.uid).toBe(15);
	});

	test('se consultan cinco direcciones como mucho', () => {
		const emails = Array.from({ length: 8 }, (_, i) => ({ label: '', value: `a${i}@x.com` }));
		expect(historyAddresses(emails)).toHaveLength(MAX_HISTORY_ADDRESSES);
		expect(MAX_HISTORY_ADDRESSES).toBe(5);
	});

	test('se pregunta de a una dirección, nunca en paralelo', async () => {
		let activas = 0;
		let maxima = 0;
		await collectHistory(['a@x', 'b@x', 'c@x'], async () => {
			activas += 1;
			maxima = Math.max(maxima, activas);
			await Promise.resolve();
			activas -= 1;
			return [];
		});
		expect(maxima).toBe(1);
	});

	test('un «no» a la primera dirección no pregunta por las demás', async () => {
		const preguntadas: string[] = [];
		await collectHistory(['a@x', 'b@x'], async (a) => {
			preguntadas.push(a);
			throw STORE_PERMISSION_DENIED;
		});
		expect(preguntadas).toEqual(['a@x']);
	});
});

/** Un `localStorage` que tira al tocarlo (modo privado, datos bloqueados). */
function withBrokenStorage<T>(run: () => T): T {
	const original = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
	Object.defineProperty(globalThis, 'localStorage', {
		configurable: true,
		get() {
			throw new Error('datos bloqueados');
		},
	});
	try {
		return run();
	} finally {
		if (original) Object.defineProperty(globalThis, 'localStorage', original);
	}
}

describe('la preferencia con el navegador de la ventana roto', () => {
	test('si no se puede leer queda apagada, y si no se puede guardar no rompe', () => {
		withBrokenStorage(() => {
			expect(loadMailHistoryEnabled()).toBe(false);
			expect(() => saveMailHistoryEnabled(true)).not.toThrow();
		});
	});

	test('con la preferencia ilegible la ficha no consulta nada', async () => {
		await withBrokenStorage(async () => {
			wrapper = mount(ContactDetail, { props: { contact: contact() } });
			await flushPromises();
		});
		expect(asked()).toHaveLength(0);
		expect(wrapper?.find('[data-testid="mail-history-off"]').exists()).toBe(true);
	});
});

let wrapper: VueWrapper | null = null;
const contact = (over: Partial<Contact> = {}) =>
	emptyContact({
		uid: 'u1',
		name: 'Ana',
		account_id: 'acc1',
		emails: [{ label: '', value: 'ana@x.com' }],
		...over,
	});
const asked = () => invokedCommands.filter((c) => c.command === 'mail_history');
const section = (id: string) => wrapper?.find(`[data-testid="mail-history-${id}"]`);

afterEach(() => {
	wrapper?.unmount();
	wrapper = null;
	localStorage.clear();
	Reflect.deleteProperty(CATALOGO.es, 'mailHistory.from');
	olvidarTodo();
});

describe('la sección según lo que contesta el servicio', () => {
	test('sin mensajes dice que no hay correos', async () => {
		saveMailHistoryEnabled(true);
		answerCommand('mail_history', () => []);
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(section('empty')?.exists()).toBe(true);
		expect(section('list')?.exists()).toBe(false);
	});

	test('un fallo cualquiera dice «no disponible» y no muestra el error crudo', async () => {
		saveMailHistoryEnabled(true);
		answerCommand('mail_history', () => {
			throw 'MessagesWith: org.freedesktop.DBus.Error.ServiceUnknown: detalle interno';
		});
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(section('unavailable')?.exists()).toBe(true);
		expect(wrapper.text()).not.toContain('ServiceUnknown');
		expect(wrapper.text()).not.toContain('detalle interno');
	});

	test('un contacto sin cuenta o sin correo no consulta ni muestra la sección', async () => {
		saveMailHistoryEnabled(true);
		wrapper = mount(ContactDetail, { props: { contact: contact({ account_id: '' }) } });
		await flushPromises();
		expect(asked()).toHaveLength(0);
		expect(wrapper.find('[data-testid="mail-history"]').exists()).toBe(false);
		wrapper.unmount();
		wrapper = mount(ContactDetail, { props: { contact: contact({ emails: [] }) } });
		await flushPromises();
		expect(asked()).toHaveLength(0);
		expect(wrapper.find('[data-testid="mail-history"]').exists()).toBe(false);
	});

	test('encender desde la ficha consulta, y apagar deja de mostrar los mensajes', async () => {
		answerCommand('mail_history', () => [mail(1, 5, 'Hola')]);
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(asked()).toHaveLength(0);
		await wrapper.find('[data-testid="mail-history-toggle"]').trigger('click');
		await flushPromises();
		expect(asked()).toHaveLength(1);
		expect(section('list')?.text()).toContain('Hola');
		expect(loadMailHistoryEnabled()).toBe(true);
		await wrapper.find('[data-testid="mail-history-toggle"]').trigger('click');
		await flushPromises();
		expect(section('list')?.exists()).toBe(false);
		expect(section('off')?.exists()).toBe(true);
		expect(loadMailHistoryEnabled()).toBe(false);
	});
});

describe('el «no» del permiso', () => {
	test('con el «no» no se vuelve a consultar al cambiar de ficha', async () => {
		saveMailHistoryEnabled(true);
		answerCommand('mail_history', () => {
			throw STORE_PERMISSION_DENIED;
		});
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		expect(asked()).toHaveLength(1);
		await wrapper.setProps({
			contact: contact({ uid: 'u2', emails: [{ label: '', value: 'luis@x.com' }] }),
		});
		await flushPromises();
		expect(asked()).toHaveLength(1);
		expect(section('denied')?.exists()).toBe(true);
	});

	test('«Pedir permiso de nuevo» pide el recurso del historial y vuelve a consultar', async () => {
		saveMailHistoryEnabled(true);
		let sinPermiso = true;
		answerCommand('mail_history', () => {
			if (sinPermiso) throw STORE_PERMISSION_DENIED;
			return [mail(1, 5, 'Hola')];
		});
		answerCommand('request_mail_history_permission_again', () => {
			sinPermiso = false;
		});
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		await wrapper.find('[data-testid="mail-history-denied"] button').trigger('click');
		await flushPromises();
		expect(
			invokedCommands.some((c) => c.command === 'request_mail_history_permission_again')
		).toBe(true);
		expect(asked()).toHaveLength(2);
		expect(section('list')?.text()).toContain('Hola');
	});

	test('si no se pudo volver a pedir se avisa con un texto traducido, no con la marca', async () => {
		saveMailHistoryEnabled(true);
		answerCommand('mail_history', () => {
			throw STORE_PERMISSION_DENIED;
		});
		answerCommand('request_mail_history_permission_again', () => {
			throw 'reask-limit-exceeded';
		});
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		await wrapper.find('[data-testid="mail-history-denied"] button').trigger('click');
		await flushPromises();
		expect(section('denied')?.text()).toContain('permiso.limite');
		expect(section('denied')?.text()).not.toContain('reask-limit-exceeded');
	});
});

describe('texto ajeno: asunto y remitente se dibujan como texto, nunca como HTML', () => {
	test('un asunto y un remitente con etiquetas se ven literales y no crean elementos', async () => {
		saveMailHistoryEnabled(true);
		const asunto = '<img src=x onerror="window.__xss=1"><script>window.__xss=2</script>';
		const remitente = '<b id="inyectado">x</b>@evil.com';
		// El catálogo de las pruebas es de una sola clave: sin ésta el remitente no se ve.
		Object.assign(CATALOGO.es, { 'mailHistory.from': 'De {0}' });
		answerCommand('mail_history', () => [mail(1, 1_700_000_000, asunto, remitente)]);
		wrapper = mount(ContactDetail, { props: { contact: contact() } });
		await flushPromises();
		const lista = section('list');
		expect(lista?.exists()).toBe(true);
		expect(lista?.text()).toContain(asunto);
		expect(lista?.text()).toContain(remitente);
		expect(lista?.find('img').exists()).toBe(false);
		expect(lista?.find('script').exists()).toBe(false);
		expect(lista?.find('#inyectado').exists()).toBe(false);
		expect((globalThis as Record<string, unknown>).__xss).toBeUndefined();
	});

	test('la ficha y el estado del historial no usan v-html', () => {
		const raiz = join(import.meta.dir, '..', 'src');
		for (const archivo of [
			'components/contacts/ContactDetail.vue',
			'composables/use-mail-history.ts',
			'tools/mail-history.ts',
		]) {
			expect(readFileSync(join(raiz, archivo), 'utf8')).not.toMatch(/v-html|innerHTML/);
		}
	});
});
