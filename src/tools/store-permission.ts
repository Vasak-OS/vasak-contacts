/**
 * El permiso del almacén de contactos, visto desde la ventana.
 *
 * El programa de Rust no muestra texto: cuando la persona negó el almacén, o
 * cuando no se pudo volver a pedir el permiso, devuelve una de estas marcas y la
 * ventana las traduce. Así el mensaje sale en el idioma de la sesión y no en el
 * que estaba compilado el backend, y la ventana decide qué pintar —el estado de
 * «sin permiso» con su botón, o un aviso al pie— sin leer una frase traducida.
 *
 * Las marcas tienen que coincidir letra por letra con las de
 * `src-tauri/src/commands.rs`.
 */

/** Lo que devuelve `contactos_de_la_cuenta` cuando la persona negó el almacén. */
export const STORE_PERMISSION_DENIED = 'store-permission-denied';

/** Si el error de un comando es ese «no» al almacén. */
export function isStorePermissionDenied(error: unknown): boolean {
	return String(error) === STORE_PERMISSION_DENIED;
}

/** Se pidió volver a abrir el diálogo hace menos de un minuto. */
export const REASK_LIMIT = 'reask-limit-exceeded';
/** El recurso no es de esta ventana, así que no puede re-pedirlo. */
export const REASK_DENIED = 'reask-access-denied';
/** No se pudo ni pedir: el servicio de permisos caído, o el bus cerrado. */
export const REASK_FAILED = 'reask-failed';

/**
 * La clave de traducción para la marca de error de volver a pedir el permiso.
 * Una marca desconocida cae en el mensaje genérico, nunca en algo crudo.
 */
export function reaskNoticeKey(code: unknown): string {
	switch (String(code)) {
		case REASK_LIMIT:
			return 'permiso.limite';
		case REASK_DENIED:
			return 'permiso.denegado';
		default:
			return 'permiso.error';
	}
}
