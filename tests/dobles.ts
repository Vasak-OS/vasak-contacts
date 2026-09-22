/**
 * Los dobles de lo que sólo existe adentro de la ventana de Tauri.
 *
 * Sin ellos, importar el layout falla en la primera línea: el marco pide
 * iconos, escucha el cambio de tema y lee la configuración del escritorio.
 */

export const laVentanaRecibio: string[] = [];

export function getCurrentWindow() {
	return {
		minimize: async () => void laVentanaRecibio.push('minimize'),
		toggleMaximize: async () => void laVentanaRecibio.push('toggleMaximize'),
		close: async () => void laVentanaRecibio.push('close'),
	};
}

/** Lo que el marco lee para saber de qué lado va la barra. */
let configuracion: Record<string, unknown> = {};

export function ponerLaConfiguracion(nueva: Record<string, unknown>) {
	configuracion = nueva;
}

export async function readConfig() {
	return configuracion;
}

export function useConfigStore() {
	return { config: configuracion, loadConfig: async () => {} };
}

/**
 * El catálogo que el proceso de Rust contestaría, con una sola clave.
 *
 * Vive acá y no dentro de `traducciones.test.ts` porque **hay un solo `invoke`
 * doblado para toda la suite**. Con dos `mock.module` sobre
 * `@tauri-apps/api/core` —el del `preload` y el de una prueba— gana el último
 * que se registra, y Bun no garantiza en qué orden evalúa los archivos: la
 * carga del idioma pasaba por el doble equivocado y `locale` quedaba
 * `undefined`. Local pasaba y en CI fallaba.
 */
export const CATALOGO = { es: { 'vsk.prueba': 'Traducido' } };

export async function invoke(comando: string) {
	if (comando === 'plugin:i18n|load_translations') return CATALOGO;
	if (comando === 'plugin:i18n|get_locale') return 'es';
	return undefined;
}

/** Los oyentes registrados por evento, para poder dispararlos desde una prueba. */
const listeners = new Map<string, Set<(evento: { payload: unknown }) => unknown>>();

export async function listen(nombre: string, manejador: (evento: { payload: unknown }) => unknown) {
	const suyos = listeners.get(nombre) ?? new Set();
	suyos.add(manejador);
	listeners.set(nombre, suyos);
	return () => {
		suyos.delete(manejador);
	};
}

/** Dispara un evento del backend y espera a que lo atiendan. */
export async function emit(nombre: string, payload: unknown = null) {
	for (const manejador of [...(listeners.get(nombre) ?? [])]) {
		await manejador({ payload });
	}
}

/**
 * El tema resuelto, de mentira.
 *
 * Devolvían la cadena vacía, y con el `<img>` escrito a mano eso daba un `img`
 * igual —vacío, pero presente—. `ThemeIcon` no dibuja el `img` hasta tener
 * fuente: deja un hueco del mismo tamaño para que la fila no salte. Así que el
 * doble tiene que devolver algo, o lo que se comprueba es el hueco.
 */
/**
 * Lo que el tema contesta para un nombre, cuando la prueba lo dice.
 *
 * Sin esto el doble contesta siempre lo mismo y un cambio de tema no se puede
 * comprobar: la fuente sale igual antes y después, así que la prueba pasaría
 * con el componente desconectado del tema.
 */
const themeIcons = new Map<string, string>();

/** Pone —o cambia— lo que el tema devuelve para un nombre. */
export function setThemeIcon(nombre: string, fuente: string) {
	themeIcons.set(nombre, fuente);
}

export async function getIconSource(nombre: string) {
	return themeIcons.get(nombre) ?? `icono:${nombre}`;
}

export async function getSymbolSource(nombre: string) {
	return themeIcons.get(nombre) ?? `simbolo:${nombre}`;
}

export function olvidarTodo() {
	laVentanaRecibio.length = 0;
	configuracion = {};
	listeners.clear();
	themeIcons.clear();
}
