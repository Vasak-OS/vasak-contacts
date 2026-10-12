/**
 * La agenda lee del almacén del sistema (`vasak-contacts#51`): el botón
 * «Actualizar» le pide una vuelta al sincronizador y relee, y el aviso
 * `contacts-changed` que pasa el programa relee solo.
 *
 * `invoke` y `listen` son los dobles de `tests/dobles.ts`: lo que hace el
 * programa del otro lado —el bus, el almacén— se prueba en Rust
 * (`src-tauri/src/store.rs`).
 */

import { afterEach, describe, expect, spyOn, test } from 'bun:test';
import { ActionButton } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { type EffectScope, effectScope } from 'vue';
import { useAgenda } from '@/composables/use-agenda';
import { emptyContact } from '@/tools/address-book';
import AddressBookView from '@/views/AddressBookView.vue';
import { answerCommand, emit, invokedCommands, listenerCount, olvidarTodo } from './dobles';

const STORE_EVENT = 'contacts-changed';

const ANA = emptyContact({
	name: 'Ana Pérez',
	sort_key: 'Pérez, Ana',
	url: 'store:a1:https://x/ana.vcf',
});
const BRUNO = emptyContact({
	name: 'Bruno Álvarez',
	sort_key: 'Álvarez, Bruno',
	url: 'store:a2:https://x/bruno.vcf',
});

const ACCOUNTS = [
	{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
	{ id: 'a2', nombre: 'bruno@otra.ejemplo.com', necesita_reconectarse: false },
];

let scope: EffectScope | null = null;
let mounted: VueWrapper | null = null;

afterEach(() => {
	scope?.stop();
	scope = null;
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
	document.body.innerHTML = '';
});

/** La agenda dentro de su propio alcance, como la usa una vista. */
function agenda() {
	scope = effectScope();
	const result = scope.run(() => useAgenda(() => 'es'));
	if (!result) throw new Error('el alcance no devolvió la agenda');
	return result;
}

/** Los comandos pedidos, sin los del idioma. */
function commands(): string[] {
	return invokedCommands.map((c) => c.command);
}

/** Cada cuenta contesta con sus contactos. */
function answerAccounts(byAccount: Record<string, ReturnType<typeof emptyContact>[]>) {
	answerCommand('listar_cuentas', () => ACCOUNTS);
	answerCommand('contactos_de_la_cuenta', (args) => ({
		libretas: [],
		contactos: byAccount[String(args?.accountId)] ?? [],
		fallos: [],
	}));
}

describe('las cuentas', () => {
	for (const [label, answer] of [
		['null', null],
		['un objeto', { error: 'no hay servicio' }],
		['un texto', 'cuentas'],
	] as const) {
		test(`una respuesta de listar_cuentas que no es una lista (${label}) es ninguna cuenta, sin aviso`, async () => {
			answerCommand('listar_cuentas', () => answer);
			const { cuentas, contactos, avisos, cargar } = agenda();

			await cargar();

			expect(cuentas.value).toEqual([]);
			expect(contactos.value).toEqual([]);
			expect(avisos.value).toEqual([]);
			expect(commands()).not.toContain('contactos_de_la_cuenta');
		});
	}
});

describe('refresh', () => {
	test('pide una vuelta de cada cuenta al sincronizador y después relee', async () => {
		answerAccounts({ a1: [ANA] });
		const { contactos, cargar, refresh } = agenda();
		await cargar();
		answerAccounts({ a1: [ANA], a2: [BRUNO] });
		invokedCommands.length = 0;

		await refresh();

		const sync = invokedCommands.find((c) => c.command === 'request_sync');
		expect(sync?.args).toEqual({ accountIds: ['a1', 'a2'] });
		const order = commands();
		expect(order.indexOf('request_sync')).toBeLessThan(order.indexOf('listar_cuentas'));
		expect(contactos.value.map((c) => c.name)).toEqual(['Ana Pérez', 'Bruno Álvarez']);
	});

	test('no le pide una vuelta a la cuenta que hay que reconectar', async () => {
		answerCommand('listar_cuentas', () => [
			{ id: 'a1', nombre: 'ana', necesita_reconectarse: false },
			{ id: 'a2', nombre: 'bruno', necesita_reconectarse: true },
		]);
		answerCommand('contactos_de_la_cuenta', () => ({ libretas: [], contactos: [], fallos: [] }));
		const { cargar, refresh } = agenda();
		await cargar();
		invokedCommands.length = 0;

		await refresh();

		const sync = invokedCommands.find((c) => c.command === 'request_sync');
		expect(sync?.args).toEqual({ accountIds: ['a1'] });
	});

	test('relee aunque pedir la vuelta falle', async () => {
		const quiet = spyOn(console, 'error').mockImplementation(() => {});
		try {
			answerAccounts({ a1: [ANA] });
			const { contactos, avisos, cargar, refresh } = agenda();
			await cargar();
			answerCommand('request_sync', () => {
				throw new Error('el sincronizador no está');
			});
			answerAccounts({ a1: [ANA], a2: [BRUNO] });

			await refresh();

			expect(contactos.value.map((c) => c.name)).toEqual(['Ana Pérez', 'Bruno Álvarez']);
			// No pudo pedir una vuelta, pero lo que hay se ve: no es un fallo de la agenda.
			expect(avisos.value).toEqual([]);
		} finally {
			quiet.mockRestore();
		}
	});

	test('el botón de actualizar de la ventana pide una vuelta y relee', async () => {
		answerAccounts({ a1: [ANA] });
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();
		answerAccounts({ a1: [ANA], a2: [BRUNO] });
		invokedCommands.length = 0;

		const refresh = mounted
			.findAllComponents(ActionButton)
			.find((b) => b.props('icon') === 'view-refresh');
		expect(refresh).toBeDefined();
		await refresh?.trigger('click');
		await flushPromises();

		expect(commands()).toContain('request_sync');
		expect(commands()).toContain('listar_cuentas');
		expect(mounted.text()).toContain('Bruno Álvarez');
	});
});

describe('el aviso del almacén', () => {
	test('contacts-changed relee la agenda sola', async () => {
		answerAccounts({ a1: [ANA] });
		const { contactos, cargar } = agenda();
		await cargar();
		await flushPromises();
		answerAccounts({ a1: [ANA], a2: [BRUNO] });

		await emit(STORE_EVENT);
		await flushPromises();

		expect(contactos.value.map((c) => c.name)).toEqual(['Ana Pérez', 'Bruno Álvarez']);
	});

	test('al cerrar la vista se deja de escuchar', async () => {
		answerAccounts({ a1: [ANA] });
		agenda();
		await flushPromises();
		expect(listenerCount(STORE_EVENT)).toBe(1);

		scope?.stop();
		scope = null;
		invokedCommands.length = 0;
		await emit(STORE_EVENT);
		await flushPromises();

		expect(listenerCount(STORE_EVENT)).toBe(0);
		expect(commands()).toEqual([]);
	});

	test('cerrada antes de terminar de engancharse, igual se deja de escuchar', async () => {
		answerAccounts({ a1: [ANA] });
		// Una que sigue abierta sí escucha: lo que se mira abajo no es que nunca
		// se enganchó.
		const open = effectScope();
		open.run(() => useAgenda(() => 'es'));
		// Se cierra en el mismo tic, antes de que `listen` conteste.
		const closed = effectScope();
		closed.run(() => useAgenda(() => 'es'));
		closed.stop();
		await flushPromises();

		expect(listenerCount(STORE_EVENT)).toBe(1);
		open.stop();
		expect(listenerCount(STORE_EVENT)).toBe(0);
	});
});
