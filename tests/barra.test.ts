/**
 * La barra de la ventana de la agenda.
 *
 * Tenía su propio marco, su propia barra y un absoluto propio para poner el
 * buscador en el medio. Los tres salen ahora de la librería.
 *
 * Lo que se comprueba es dónde queda cada cosa, que es lo que se rompe al
 * mudarla. La cadena es larga —la vista se la pasa al layout, el layout al
 * marco, el marco a la barra— y basta con que uno de los tres no reexponga una
 * ranura para que lo que se le ponga desaparezca sin ningún error.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import {
	AppBar,
	BarSearch,
	olvidarLosIconosDelTema,
	WindowControls,
	WindowFrame,
} from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ListaComponent from '@/components/agenda/ListaComponent.vue';
import AgendaView from '@/views/AgendaView.vue';
import { olvidarTodo } from './dobles';

let vista: VueWrapper | null = null;

function abrir() {
	vista = mount(AgendaView);
	return vista;
}

/** Lo que cada llamada a `ranura()` dejó montado, para desmontarlo después. */
const sueltos: VueWrapper[] = [];

/**
 * Lo que se dibuja dentro de una ranura de la barra.
 *
 * Monta un componente aparte, así que lo que devuelve **no** cuelga de `vista`
 * y no se va con ella: se anota acá y el `afterEach` lo desmonta. Sin eso cada
 * prueba deja un componente vivo, con sus oyentes puestos, hasta que termina el
 * archivo.
 */
function ranura(ventana: VueWrapper, nombre: string) {
	const barra = ventana.findComponent(AppBar);
	const dibujar = (barra.vm.$slots as Record<string, (() => unknown) | undefined>)[nombre];
	if (!dibujar) return null;
	const suelto = mount({ render: () => dibujar() });
	sueltos.push(suelto);
	return suelto;
}

afterEach(() => {
	for (const suelto of sueltos.splice(0)) suelto.unmount();
	vista?.unmount();
	vista = null;
	olvidarTodo();
	// Lo que el tema resolvió se memoriza en el módulo de la librería, y un
	// módulo se comparte entre archivos de prueba: sin vaciarlo, el primero que
	// pida un icono con el tema sin preparar deja guardado que no hay ninguno.
	olvidarLosIconosDelTema();
});

describe('la ventana', () => {
	test('usa el marco compartido', () => {
		expect(abrir().findComponent(WindowFrame).exists()).toBe(true);
	});

	test('y no queda un segundo borde dibujado a mano', () => {
		expect(abrir().findAll('.rounded-corner-window').length).toBe(1);
	});

	test('con los tres botones', () => {
		expect(abrir().findComponent(WindowControls).findAll('button').length).toBe(3);
	});
});

describe('lo que va en la barra', () => {
	test('el icono va en `identidad`', async () => {
		// En el contenido de la barra se desplazaría con lo demás cuando queda a
		// un costado: `identidad` es la única zona que no scrollea.
		//
		// Lo dibuja `ThemeIcon`, que lo resuelve contra el tema: hasta que
		// vuelve deja un hueco del tamaño del icono y no un `img`.
		const dentro = ranura(abrir(), 'identidad');
		await new Promise((listo) => setTimeout(listo, 0));
		await nextTick();

		expect(dentro?.find('img').attributes('alt')).toBe('app.nombre');
	});

	test('actualizar va en `acciones`, que es lo pegado a los botones', () => {
		// Es donde está en el resto de las aplicaciones. Antes lo empujaba hasta
		// ahí un `span` con `flex-1`; ahora el hueco lo pone la barra sola.
		const dentro = ranura(abrir(), 'acciones');

		expect(dentro?.find('[aria-label="lista.actualizar"]').exists()).toBe(true);
	});

	test('y no quedó ningún hueco a mano empujando cosas', () => {
		expect(abrir().findComponent(AppBar).findAll('span.flex-1').length).toBe(0);
	});
});

describe('el buscador', () => {
	test('es el de la librería y no un campo suelto', () => {
		// Un `<input>` a secas no se puede plegar, y con la barra a un costado
		// hay cuarenta y ocho píxeles de ancho: ahí un campo de texto no se lee
		// ni se escribe.
		expect(abrir().findComponent(BarSearch).exists()).toBe(true);
	});

	test('va en el contenido de la barra, que es lo único que crece', () => {
		// Y no en `centro`, que centra respecto de la ventana entera: con el
		// icono de un lado y el estado, actualizar y los tres controles del
		// otro, el medio de la ventana no es el medio del hueco.
		const dentro = ranura(abrir(), 'default');

		expect(dentro?.findComponent(BarSearch).exists()).toBe(true);
	});

	test('y ya no queda nada en `centro`', () => {
		// La ranura sigue existiendo en el marco, y llenar las dos pondría dos
		// buscadores: el de `centro` va encima de la barra, así que se verían
		// los dos a la vez y superpuestos.
		expect(ranura(abrir(), 'centro')).toBeNull();
	});

	test('centrado en el hueco, con márgenes automáticos', () => {
		// `m-auto` reparte lo que sobra del contenedor a los dos lados. Sin él
		// el grupo se pega al principio de la barra: el contenedor es flexible
		// y los hijos no se centran solos.
		//
		// En los dos ejes y no sólo el horizontal: con la barra a un costado el
		// hueco es vertical, y el margen automático centra en el eje que
		// corresponda sin que haya que preguntar cuál es.
		const dentro = ranura(abrir(), 'default');

		expect(dentro?.find('div').classes()).toContain('m-auto');
	});

	test('lo que se escribe llega a la lista', async () => {
		// El campo estaba atado con `v-model` a la consulta de la agenda, y al
		// cambiarlo por el componente eso pasa por `update:modelValue`: si el
		// `v-model` no quedara puesto, se escribe y no pasa nada. Se mira la
		// lista y no el propio buscador, que es el recorrido entero.
		const ventana = abrir();

		ventana.findComponent(BarSearch).vm.$emit('update:modelValue', 'pepe');
		await ventana.vm.$nextTick();

		expect(ventana.findComponent(ListaComponent).props('consulta')).toBe('pepe');
	});

	test('el contador va al lado y cuenta para el centrado', () => {
		// Lo que se ve como una sola cosa es «campo más número»: colgado en
		// absoluto no contaba, y el conjunto quedaba corrido aunque el campo
		// estuviera centrado.
		const dentro = ranura(abrir(), 'default');
		const contador = dentro?.find('[aria-live="polite"]');

		expect(contador?.classes()).not.toContain('absolute');
	});

	test('y no se mueve al cambiar de dos dígitos a tres', () => {
		// Sin un ancho mínimo, pasar de «9 contactos» a «124 contactos» corre el
		// campo: ahora el número cuenta para el centrado, así que cada dígito lo
		// empuja. Es lo que este ancho —y las cifras de ancho fijo— sostienen.
		const dentro = ranura(abrir(), 'default');
		const contador = dentro?.find('[aria-live="polite"]');

		expect(contador?.classes()).toContain('min-w-24');
		expect(contador?.classes()).toContain('tabular-nums');
	});
});
