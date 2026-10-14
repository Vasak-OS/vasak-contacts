/**
 * La agenda lee del almacén del sistema (`vasak-contacts#51`): el botón
 * «Actualizar» le pide una vuelta al sincronizador y relee, y el aviso de
 * cambios del plugin del almacén (`CHANGED_EVENT`) relee solo.
 *
 * `invoke` y `listen` son los dobles de `tests/dobles.ts`, por debajo del
 * paquete `@vasakgroup/plugin-vsk-store`: el bus y la comprobación del dueño
 * se prueban en el plugin, y la lectura del almacén en Rust
 * (`src-tauri/src/store.rs`).
 */

import { afterEach, describe, expect, spyOn, test } from 'bun:test';
import { CHANGED_EVENT } from '@vasakgroup/plugin-vsk-store';
import { ActionButton } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { type EffectScope, effectScope } from 'vue';
import { useAgenda } from '@/composables/use-agenda';
import { emptyContact } from '@/tools/address-book';
import { bookKey, loadHiddenBooks } from '@/tools/library-filter';
import { REASK_LIMIT, STORE_PERMISSION_DENIED } from '@/tools/store-permission';
import AddressBookView from '@/views/AddressBookView.vue';
import { answerCommand, emit, invokedCommands, listenerCount, olvidarTodo } from './dobles';

/** El comando que vuelve a abrir el diálogo del permiso del almacén. */
const REQUEST_AGAIN = 'request_store_permission_again';

/** El pedido de «Actualizar» del plugin del almacén. */
const REQUEST_SYNC = 'plugin:vsk-store|request_sync';

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
	globalThis.localStorage?.clear();
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

		const sync = invokedCommands.find((c) => c.command === REQUEST_SYNC);
		expect(sync?.args).toEqual({ accountIds: ['a1', 'a2'] });
		const order = commands();
		expect(order.indexOf(REQUEST_SYNC)).toBeLessThan(order.indexOf('listar_cuentas'));
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

		const sync = invokedCommands.find((c) => c.command === REQUEST_SYNC);
		expect(sync?.args).toEqual({ accountIds: ['a1'] });
	});

	test('relee aunque pedir la vuelta falle', async () => {
		const quiet = spyOn(console, 'error').mockImplementation(() => {});
		try {
			answerAccounts({ a1: [ANA] });
			const { contactos, avisos, cargar, refresh } = agenda();
			await cargar();
			answerCommand(REQUEST_SYNC, () => {
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

		expect(commands()).toContain(REQUEST_SYNC);
		expect(commands()).toContain('listar_cuentas');
		expect(mounted.text()).toContain('Bruno Álvarez');
	});
});

describe('el aviso del almacén', () => {
	test('el aviso de cambios del almacén relee la agenda sola', async () => {
		answerAccounts({ a1: [ANA] });
		const { contactos, cargar } = agenda();
		await cargar();
		await flushPromises();
		answerAccounts({ a1: [ANA], a2: [BRUNO] });

		await emit(CHANGED_EVENT, { areas: ['contacts'] });
		await flushPromises();

		expect(contactos.value.map((c) => c.name)).toEqual(['Ana Pérez', 'Bruno Álvarez']);
	});

	test('al cerrar la vista se deja de escuchar', async () => {
		answerAccounts({ a1: [ANA] });
		agenda();
		await flushPromises();
		expect(listenerCount(CHANGED_EVENT)).toBe(1);

		scope?.stop();
		scope = null;
		invokedCommands.length = 0;
		await emit(CHANGED_EVENT, { areas: ['contacts'] });
		await flushPromises();

		expect(listenerCount(CHANGED_EVENT)).toBe(0);
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

		expect(listenerCount(CHANGED_EVENT)).toBe(1);
		open.stop();
		expect(listenerCount(CHANGED_EVENT)).toBe(0);
	});
});

describe('elegir qué libretas se ven y filtrar por categoría', () => {
	const ANA_P = emptyContact({
		name: 'Ana Pérez',
		sort_key: 'Pérez, Ana',
		url: 'store:a1:https://x/ana.vcf',
		address_book_id: 'personal',
		categories: ['Trabajo'],
	});
	const CARLA_T = emptyContact({
		name: 'Carla Soto',
		sort_key: 'Soto, Carla',
		url: 'store:a1:https://x/carla.vcf',
		address_book_id: 'trabajo',
		categories: ['Trabajo', 'Clientes'],
	});
	const BRUNO_P = emptyContact({
		name: 'Bruno Álvarez',
		sort_key: 'Álvarez, Bruno',
		url: 'store:a2:https://x/bruno.vcf',
		address_book_id: 'personal',
		categories: ['Amigos'],
	});

	/** Cada cuenta contesta con sus libretas y sus contactos. */
	function answerWithBooks() {
		answerCommand('listar_cuentas', () => ACCOUNTS);
		answerCommand('contactos_de_la_cuenta', (args) => {
			switch (String(args?.accountId)) {
				case 'a1':
					return {
						libretas: [
							{ url: 'personal', nombre: 'Personal' },
							{ url: 'trabajo', nombre: 'Trabajo' },
						],
						contactos: [ANA_P, CARLA_T],
						fallos: [],
					};
				case 'a2':
					return {
						libretas: [{ url: 'personal', nombre: 'Personal' }],
						contactos: [BRUNO_P],
						fallos: [],
					};
				default:
					return { libretas: [], contactos: [], fallos: [] };
			}
		});
	}

	test('junta las libretas de todas las cuentas, con la cuenta en la clave', async () => {
		answerWithBooks();
		const { addressBooks, cargar } = agenda();
		await cargar();

		expect(addressBooks.value.map((b) => b.key)).toEqual([
			bookKey('a1', 'personal'),
			bookKey('a1', 'trabajo'),
			bookKey('a2', 'personal'),
		]);
	});

	test('tagua cada contacto con su cuenta: dos libretas con el mismo id no se confunden', async () => {
		answerWithBooks();
		const { visibles, cargar } = agenda();
		await cargar();

		const ana = visibles.value.find((c) => c.name === 'Ana Pérez');
		const bruno = visibles.value.find((c) => c.name === 'Bruno Álvarez');
		expect(ana?.account_id).toBe('a1');
		expect(bruno?.account_id).toBe('a2');
	});

	test('ocultar una libreta saca a los suyos y lo recuerda', async () => {
		answerWithBooks();
		const { visibles, cargar, toggleBook } = agenda();
		await cargar();
		expect(visibles.value.map((c) => c.name)).toContain('Carla Soto');

		toggleBook(bookKey('a1', 'trabajo'));

		// Ordenados por apellido: Álvarez antes que Pérez.
		expect(visibles.value.map((c) => c.name)).toEqual(['Bruno Álvarez', 'Ana Pérez']);
		// Y se guardó, para la próxima vez que se abra.
		expect(loadHiddenBooks().has(bookKey('a1', 'trabajo'))).toBe(true);
	});

	test('una libreta oculta de antes se aplica al abrir', async () => {
		// La preferencia vive en el navegador de la ventana: una sesión anterior
		// la escribió, y al abrir ya no se ve esa libreta.
		globalThis.localStorage?.setItem(
			'vasak-contacts.hidden-address-books',
			JSON.stringify([bookKey('a2', 'personal')])
		);
		answerWithBooks();
		const { visibles, cargar } = agenda();
		await cargar();

		expect(visibles.value.map((c) => c.name)).not.toContain('Bruno Álvarez');
	});

	test('las categorías salen de las libretas que se ven', async () => {
		answerWithBooks();
		const { categories, cargar, toggleBook } = agenda();
		await cargar();
		// Están las de todas las libretas, ordenadas y sin repetir.
		expect(categories.value).toEqual(['Amigos', 'Clientes', 'Trabajo']);

		// Al ocultar «trabajo», sus categorías exclusivas se van de la lista.
		toggleBook(bookKey('a1', 'trabajo'));
		expect(categories.value).toEqual(['Amigos', 'Trabajo']);
	});

	test('filtrar por categoría muestra a quien esté en alguna, y se puede limpiar', async () => {
		answerWithBooks();
		const { visibles, cargar, toggleCategory, clearCategories } = agenda();
		await cargar();

		toggleCategory('Amigos');
		expect(visibles.value.map((c) => c.name)).toEqual(['Bruno Álvarez']);

		toggleCategory('Clientes');
		// Ordenados por apellido: Álvarez antes que Soto. Ana queda afuera: sólo
		// tiene «Trabajo».
		expect(visibles.value.map((c) => c.name)).toEqual(['Bruno Álvarez', 'Carla Soto']);

		clearCategories();
		expect(visibles.value).toHaveLength(3);
	});
});

describe('sin permiso para leer el almacén', () => {
	/** Las cuentas están, pero el almacén le dice que no a leer los contactos. */
	function answerDenied() {
		answerCommand('listar_cuentas', () => ACCOUNTS);
		answerCommand('contactos_de_la_cuenta', () => {
			throw STORE_PERMISSION_DENIED;
		});
	}

	test('el «no» del almacén deja la agenda en «sin permiso», sin aviso por cuenta', async () => {
		answerDenied();
		const { sinPermiso, contactos, avisos, cargar } = agenda();

		await cargar();

		expect(sinPermiso.value).toBe(true);
		expect(contactos.value).toEqual([]);
		// El almacén es uno solo: no se repite el mismo «no» por cada cuenta.
		expect(avisos.value).toEqual([]);
	});

	test('volver a pedir el permiso lo pide y relee; con el permiso dado, se apaga', async () => {
		answerDenied();
		answerCommand(REQUEST_AGAIN, () => undefined);
		const { sinPermiso, contactos, cargar, pedirPermisoDeNuevo } = agenda();
		await cargar();
		expect(sinPermiso.value).toBe(true);

		// La persona concede: ahora el almacén contesta.
		answerCommand('contactos_de_la_cuenta', (args) => ({
			libretas: [],
			contactos: String(args?.accountId) === 'a1' ? [ANA] : [BRUNO],
			fallos: [],
		}));
		invokedCommands.length = 0;

		const error = await pedirPermisoDeNuevo();

		expect(error).toBeNull();
		// Pidió volver a abrir el diálogo y después releyó.
		expect(commands()).toContain(REQUEST_AGAIN);
		expect(commands()).toContain('listar_cuentas');
		expect(sinPermiso.value).toBe(false);
		expect(contactos.value.map((c) => c.name)).toEqual(['Ana Pérez', 'Bruno Álvarez']);
	});

	test('si no se pudo ni pedir, devuelve la marca y no relee', async () => {
		answerDenied();
		answerCommand(REQUEST_AGAIN, () => {
			throw REASK_LIMIT;
		});
		const { sinPermiso, cargar, pedirPermisoDeNuevo } = agenda();
		await cargar();
		invokedCommands.length = 0;

		const error = await pedirPermisoDeNuevo();

		expect(error).toBe(REASK_LIMIT);
		// Sólo intentó pedir: sin reintento, la agenda sigue sin permiso.
		expect(commands()).toEqual([REQUEST_AGAIN]);
		expect(sinPermiso.value).toBe(true);
	});

	test('la vista muestra el botón, y al apretarlo pide y relee', async () => {
		answerDenied();
		answerCommand(REQUEST_AGAIN, () => undefined);
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();

		const boton = mounted
			.findAllComponents(ActionButton)
			.find((b) => b.props('label') === 'permiso.pedir');
		expect(boton).toBeDefined();

		// La persona concede antes de apretar.
		answerCommand('contactos_de_la_cuenta', () => ({ libretas: [], contactos: [ANA], fallos: [] }));
		invokedCommands.length = 0;

		await boton?.trigger('click');
		await flushPromises();

		expect(commands()).toContain(REQUEST_AGAIN);
		expect(commands()).toContain('listar_cuentas');
		expect(mounted.text()).toContain('Ana Pérez');
	});

	test('la vista dice por qué no se pudo volver a pedir', async () => {
		answerDenied();
		answerCommand(REQUEST_AGAIN, () => {
			throw REASK_LIMIT;
		});
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		await flushPromises();

		const boton = mounted
			.findAllComponents(ActionButton)
			.find((b) => b.props('label') === 'permiso.pedir');
		await boton?.trigger('click');
		await flushPromises();

		// Sin catálogo, `t()` devuelve la clave: alcanza para ver que el aviso del
		// techo se mostró y no el genérico.
		expect(mounted.text()).toContain('permiso.limite');
	});
});
