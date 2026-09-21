/**
 * Lo que la agenda dejó de dibujar por su cuenta.
 *
 * Es la aplicación que menos tenía para adoptar: ya usaba `BarSearch` y
 * `WindowFrame`, y sus estados de carga ya se anunciaban. Quedaban dos cosas —
 * el icono reactivo, noventa y una líneas para dos iconos, y el aviso de «no se
 * pudo leer todo», que traía una copia a mano de los colores del sistema.
 *
 * Lo que se comprueba del aviso es el tono y el rol, que es lo que puede
 * separarse de la tabla compartida sin que nada falle.
 */

import { afterEach, beforeAll, describe, expect, test } from 'bun:test';
import { AlertMessage, CLASES_POR_TONO, olvidarLosIconosDelTema } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import CuentasComponent from '@/components/agenda/CuentasComponent.vue';
import { olvidarTodo } from './dobles';

const RAIZ = new URL('..', import.meta.url).pathname;

const vistas = new Set<VueWrapper>();

function anotar<T extends VueWrapper>(vista: T): T {
	vistas.add(vista);
	return vista;
}

beforeAll(async () => {
	const calentar = mount(CuentasComponent, { props: { cuentas: [], avisos: [] } });
	calentar.unmount();
}, 60_000);

afterEach(() => {
	for (const vista of vistas) vista.unmount();
	vistas.clear();
	olvidarTodo();
	olvidarLosIconosDelTema();
});

function cuentas(avisos: string[] = []) {
	return anotar(mount(CuentasComponent, { props: { cuentas: [], avisos } }));
}

describe('el aviso de lo que no se pudo leer', () => {
	test('usa el aviso del sistema y no una caja propia', async () => {
		const vista = cuentas(['no se pudo abrir /home/pato/.contactos/trabajo']);

		const aviso = vista.findComponent(AlertMessage);
		expect(aviso.exists()).toBe(true);
		expect(aviso.text()).toContain('no se pudo abrir /home/pato/.contactos/trabajo');
	});

	test('es `warning` y no `error`: la agenda anda, sólo que incompleta', async () => {
		// Y con eso el rol sigue siendo `status`, o sea que espera turno en vez
		// de interrumpir lo que el lector de pantalla esté leyendo. Un error de
		// una libreta no es una emergencia.
		const vista = cuentas(['falló una']);
		const aviso = vista.findComponent(AlertMessage);

		expect(aviso.props('tone')).toBe('warning');
		expect(aviso.attributes('role')).toBe('status');
		expect(aviso.classes().join(' ')).toContain(CLASES_POR_TONO.warning.split(' ')[0]);
	});

	test('y sin avisos no se dibuja nada', async () => {
		expect(cuentas().findComponent(AlertMessage).exists()).toBe(false);
	});
});

describe('lo que la agenda ya no dibuja', () => {
	test('el composable del icono se fue', async () => {
		expect(await Bun.file(`${RAIZ}src/composables/useReactiveIcon.ts`).exists()).toBe(false);
	});

	test('y nadie lo importa', async () => {
		const fuentes = [...new Bun.Glob('src/**/*.{vue,ts}').scanSync(RAIZ)];
		expect(fuentes.length).toBeGreaterThan(8);

		const culpables: string[] = [];
		for (const ruta of fuentes) {
			const texto = await Bun.file(`${RAIZ}${ruta}`).text();
			if (/useReactiveIcon/.test(texto)) culpables.push(ruta);
		}

		expect(culpables).toEqual([]);
	});
});
