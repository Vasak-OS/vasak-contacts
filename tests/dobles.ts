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

export async function listen(_nombre: string, _manejador: () => unknown) {
	return () => {};
}

export async function getIconSource(_nombre: string) {
	return '';
}

export async function getSymbolSource(_nombre: string) {
	return '';
}

export function olvidarTodo() {
	laVentanaRecibio.length = 0;
	configuracion = {};
}
