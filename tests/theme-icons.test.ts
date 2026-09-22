/**
 * Los iconos de la agenda siguen al tema, y la recarga va por el planificador.
 *
 * La agenda no resuelve ningún icono: los pide por nombre a `ThemeIcon`. Lo que
 * se comprueba acá es que la recarga llegue por el planificador de la librería
 * y no en el acto.
 *
 * Importa porque hasta este cambio no era así, y nada lo decía: el manifiesto
 * pedía `^1.0.0`, que admite la 1.4.0, pero `bun.lock` había quedado en la
 * 1.0.0 y se empaquetaba ésa. Un rango corregido no mueve el candado, así que
 * la agenda venía con la librería de antes del planificador sin que ninguna
 * prueba ni el CI dijeran nada.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { olvidarLosIconosDelTema } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import AgendaView from '@/views/AgendaView.vue';
import { emit, olvidarTodo, setThemeIcon } from './dobles';

/** Deja que terminen las promesas encadenadas del pedido del icono. */
async function settle(rounds = 8) {
	for (let i = 0; i < rounds; i++) {
		await nextTick();
		await new Promise((done) => setTimeout(done, 0));
	}
}

/**
 * Lo mismo, esperando además a que el planificador recargue.
 *
 * Desde la 1.3.0 el cambio de tema no vuelve a pedir en el acto: vacía la
 * memoria y **agenda** la recarga, para que el anuncio del tema de iconos y el
 * de GTK no disparen dos barridos.
 */
async function settleWithReload() {
	await new Promise((done) => setTimeout(done, 150));
	await settle();
}

let mounted: VueWrapper | null = null;

function openAgenda() {
	mounted = mount(AgendaView);
	return mounted;
}

/** El `src` del icono de la aplicación, que es el primero que dibuja la vista. */
function appIcon(vista: VueWrapper): string | undefined {
	return vista.findAll('img')[0]?.attributes('src');
}

beforeEach(() => {
	olvidarTodo();
	// La memoria de la librería vive en su módulo y sobrevive entre archivos de
	// prueba: sin vaciarla, esto ve el icono que dejó otra.
	olvidarLosIconosDelTema();
});

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarLosIconosDelTema();
});

describe('la agenda dibuja sus iconos con el tema', () => {
	test('el de la aplicación sale del tema, por nombre', async () => {
		setThemeIcon('contacts', 'data:image/svg+xml,agenda-clara');

		const vista = openAgenda();
		await settle();

		expect(appIcon(vista)).toBe('data:image/svg+xml,agenda-clara');
	});

	test('la recarga se agenda, no pasa en el acto', async () => {
		// Es lo que separa la 1.0.0 —la que se venía empaquetando— de la 1.4.0:
		// sin planificador el dibujo nuevo ya estaría acá. Con él, todavía no, y
		// por eso una ráfaga de anuncios —el tema de iconos y el de GTK llegan
		// juntos— no dispara dos barridos.
		setThemeIcon('contacts', 'data:image/svg+xml,agenda-clara');

		const vista = openAgenda();
		await settle();

		setThemeIcon('contacts', 'data:image/svg+xml,agenda-oscura');
		await emit('vicons:theme-changed');
		await settle();

		expect(appIcon(vista)).not.toBe('data:image/svg+xml,agenda-oscura');

		await settleWithReload();
		expect(appIcon(vista)).toBe('data:image/svg+xml,agenda-oscura');
	});
});
