/**
 * Cómo va la sincronización de cada cuenta, en el panel de cuentas
 * (Vasak-OS/vasak-contacts#53).
 *
 * El estado sale de `sync_status` (el `GetStatus` del almacén) y se relee con el
 * aviso `contacts-sync-status`; «Actualizar» guarda a qué cuentas no se les
 * pudo pedir una vuelta. `invoke` y `listen` son los dobles de
 * `tests/dobles.ts`: el bus y el almacén se prueban en Rust
 * (`src-tauri/src/store.rs`).
 */

import { afterEach, beforeEach, describe, expect, spyOn, test } from 'bun:test';
import { ActionButton, StatusDot } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { type EffectScope, effectScope } from 'vue';
import AccountsPanel from '@/components/contacts/AccountsPanel.vue';
import { useAgenda } from '@/composables/use-agenda';
import type { AccountSyncStatus, SyncRequestFailure } from '@/tools/sync-status';
import AddressBookView from '@/views/AddressBookView.vue';
import { answerCommand, emit, invokedCommands, listenerCount, olvidarTodo } from './dobles';

const STATUS_EVENT = 'contacts-sync-status';

const ACCOUNTS = [
	{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
	{ id: 'a2', nombre: 'bruno@otra.ejemplo.com', necesita_reconectarse: false },
];

const STATUSES: AccountSyncStatus[] = [
	{ account_id: 'a1', state: 'syncing', detail: '', last_synced_at: null },
	{ account_id: 'a2', state: 'failed', detail: 'clave rechazada', last_synced_at: null },
];

let scope: EffectScope | null = null;
let mounted: VueWrapper | null = null;

// Otros archivos montan la ventana sin soltar sus oyentes ni olvidar lo que
// pidieron: se arranca de cero para contar sólo lo de cada prueba.
beforeEach(() => {
	olvidarTodo();
});

afterEach(() => {
	scope?.stop();
	scope = null;
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
	document.body.innerHTML = '';
});

function agenda() {
	scope = effectScope();
	const result = scope.run(() => useAgenda(() => 'es'));
	if (!result) throw new Error('el alcance no devolvió la agenda');
	return result;
}

function timesInvoked(command: string): number {
	return invokedCommands.filter((c) => c.command === command).length;
}

/** Las cuentas, sin contactos: lo que se mira acá es la línea de estado. */
function answerAccounts(accounts: unknown[] = ACCOUNTS) {
	answerCommand('listar_cuentas', () => accounts);
	answerCommand('contactos_de_la_cuenta', () => ({ libretas: [], contactos: [], fallos: [] }));
}

/** Corre `fn` sin ensuciar la salida con los `console.error` esperados. */
async function quietly<T>(fn: () => Promise<T>): Promise<T> {
	const quiet = spyOn(console, 'error').mockImplementation(() => {});
	try {
		return await fn();
	} finally {
		quiet.mockRestore();
	}
}

describe('el estado de la sincronización', () => {
	test('al abrir se lee el estado de cada cuenta del almacén', async () => {
		answerCommand('sync_status', () => STATUSES);
		const { syncStatuses } = agenda();
		await flushPromises();

		expect(timesInvoked('sync_status')).toBe(1);
		expect(syncStatuses.value).toEqual(STATUSES);
	});

	test('una respuesta que no es una lista es ningún estado', async () => {
		answerCommand('sync_status', () => ({ error: 'no' }));
		const { syncStatuses } = agenda();
		await flushPromises();

		expect(syncStatuses.value).toEqual([]);
	});

	test('si no se puede leer el estado, se queda el que había', async () => {
		answerCommand('sync_status', () => STATUSES);
		const { syncStatuses } = agenda();
		await flushPromises();
		answerCommand('sync_status', () => {
			throw new Error('GetStatus: el servicio no existe');
		});

		await quietly(async () => {
			await emit(STATUS_EVENT);
			await flushPromises();
		});

		expect(timesInvoked('sync_status')).toBe(2);
		expect(syncStatuses.value).toEqual(STATUSES);
	});

	test('el aviso de estado relee el estado, no la agenda', async () => {
		answerCommand('sync_status', () => STATUSES);
		const { syncStatuses } = agenda();
		await flushPromises();
		const synced: AccountSyncStatus[] = [
			{ account_id: 'a1', state: 'synced', detail: '', last_synced_at: null },
		];
		answerCommand('sync_status', () => synced);

		await emit(STATUS_EVENT);
		await flushPromises();

		expect(syncStatuses.value).toEqual(synced);
		expect(timesInvoked('listar_cuentas')).toBe(0);
	});

	test('al cerrar la vista deja de escuchar el estado', async () => {
		agenda();
		await flushPromises();
		expect(listenerCount(STATUS_EVENT)).toBe(1);

		scope?.stop();
		scope = null;

		expect(listenerCount(STATUS_EVENT)).toBe(0);
	});

	test('cerrada antes de engancharse, tampoco queda escuchando el estado', async () => {
		const closed = effectScope();
		closed.run(() => useAgenda(() => 'es'));
		closed.stop();
		await flushPromises();

		expect(listenerCount(STATUS_EVENT)).toBe(0);
	});
});

describe('lo que «Actualizar» no pudo pedir', () => {
	test('se guarda qué cuentas rechazaron el pedido y por qué', async () => {
		answerAccounts();
		const failures: SyncRequestFailure[] = [{ account_id: 'a2', reason: 'denied' }];
		answerCommand('request_sync', () => failures);
		const { cargar, refresh, refreshFailures } = agenda();
		await cargar();

		await refresh();

		expect(refreshFailures.value).toEqual(failures);
	});

	test('un «Actualizar» que sale bien borra los rechazos del anterior', async () => {
		answerAccounts();
		answerCommand('request_sync', () => [{ account_id: 'a2', reason: 'failed' }]);
		const { cargar, refresh, refreshFailures } = agenda();
		await cargar();
		await refresh();
		answerCommand('request_sync', () => []);

		await refresh();

		expect(refreshFailures.value).toEqual([]);
	});

	test('si el programa no contesta, todas las cuentas pedidas quedan sin servicio', async () => {
		answerAccounts([...ACCOUNTS, { id: 'vencida', nombre: 'vieja', necesita_reconectarse: true }]);
		answerCommand('request_sync', () => {
			throw new Error('el programa no contesta');
		});
		const { cargar, refresh, refreshFailures } = agenda();
		await cargar();

		await quietly(() => refresh());

		// La que hay que reconectar ni se pidió: su aviso es el de reconectar.
		expect(refreshFailures.value).toEqual([
			{ account_id: 'a1', reason: 'unavailable' },
			{ account_id: 'a2', reason: 'unavailable' },
		]);
	});

	test('«Actualizar» también relee el estado', async () => {
		answerAccounts();
		const { refresh, syncStatuses } = agenda();
		await flushPromises();
		answerCommand('sync_status', () => STATUSES);

		await refresh();

		expect(syncStatuses.value).toEqual(STATUSES);
	});

	test('el aviso de estado olvida «se acaba de pedir» y deja los demás rechazos', async () => {
		answerAccounts();
		answerCommand('request_sync', () => [
			{ account_id: 'a1', reason: 'too_soon' },
			{ account_id: 'a2', reason: 'denied' },
		]);
		const { cargar, refresh, refreshFailures } = agenda();
		await cargar();
		await refresh();
		await flushPromises();

		await emit(STATUS_EVENT);
		await flushPromises();

		expect(refreshFailures.value).toEqual([{ account_id: 'a2', reason: 'denied' }]);
	});
});

describe('la línea en el panel de cuentas', () => {
	function panel(props: Record<string, unknown>) {
		mounted = mount(AccountsPanel, { props: { accounts: ACCOUNTS, notices: [], ...props } });
		return mounted;
	}

	test('cada cuenta con estado tiene su línea, anunciada como estado', () => {
		const wrapper = panel({ syncStatuses: STATUSES });

		const lines = wrapper.findAll('[data-testid="sync-status"]');
		expect(lines).toHaveLength(2);
		for (const line of lines) expect(line.attributes('role')).toBe('status');
		expect(lines[0].text()).toContain('sync.syncing');
		expect(lines[1].text()).toContain('sync.failed');
	});

	test('el punto lleva el tono y late mientras sincroniza', () => {
		const wrapper = panel({ syncStatuses: STATUSES });

		const dots = wrapper
			.findAll('[data-testid="sync-status"]')
			.map((line) => line.findComponent(StatusDot));
		expect(dots.map((d) => d.props('tone'))).toEqual(['accent', 'error']);
		expect(dots.map((d) => Boolean(d.props('pulse')))).toEqual([true, false]);
	});

	test('lo que «Actualizar» no pudo pedir se dice debajo de esa cuenta', () => {
		const wrapper = panel({
			syncStatuses: STATUSES,
			refreshFailures: [{ account_id: 'a1', reason: 'denied' }],
		});

		const lines = wrapper.findAll('[data-testid="sync-status"]');
		expect(lines[0].text()).toContain('sync.request.denied');
		expect(lines[0].findComponent(StatusDot).props('tone')).toBe('warning');
		expect(lines[1].text()).toContain('sync.failed');
	});

	test('una cuenta sin estado ni rechazo no tiene línea', () => {
		const wrapper = panel({ syncStatuses: [STATUSES[0]] });

		expect(wrapper.findAll('[data-testid="sync-status"]')).toHaveLength(1);
	});

	test('en una cuenta para reconectar gana el aviso de reconectar', () => {
		const wrapper = panel({
			accounts: [{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: true }],
			syncStatuses: STATUSES,
		});

		expect(wrapper.find('[data-testid="needs-reconnect"]').exists()).toBe(true);
		expect(wrapper.find('[data-testid="sync-status"]').exists()).toBe(false);
	});
});

describe('la ventana', () => {
	function view() {
		mounted = mount(AddressBookView, {
			attachTo: document.body.appendChild(document.createElement('div')),
		});
		return mounted;
	}

	test('muestra el estado del almacén debajo de cada cuenta', async () => {
		answerAccounts();
		answerCommand('sync_status', () => STATUSES);
		const wrapper = view();
		await flushPromises();

		const lines = wrapper.findAll('[data-testid="sync-status"]');
		expect(lines.map((l) => l.text())).toEqual(['sync.syncing', 'sync.failed']);
	});

	test('el botón de actualizar dice qué cuenta rechazó el pedido', async () => {
		answerAccounts();
		const wrapper = view();
		await flushPromises();
		expect(wrapper.find('[data-testid="sync-status"]').exists()).toBe(false);
		answerCommand('request_sync', () => [{ account_id: 'a2', reason: 'too_soon' }]);

		const refresh = wrapper
			.findAllComponents(ActionButton)
			.find((b) => b.props('icon') === 'view-refresh');
		await refresh?.trigger('click');
		await flushPromises();

		const lines = wrapper.findAll('[data-testid="sync-status"]');
		expect(lines.map((l) => l.text())).toEqual(['sync.request.too_soon']);
	});
});
