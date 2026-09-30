/**
 * Pedirle al programa la foto que está afuera de la tarjeta.
 *
 * **La ventana no pide nunca la dirección de la foto.** Pedirla desde acá es
 * un rastreador de apertura: el dueño de la dirección se entera de cuándo se
 * abrió la ficha, con las cabeceras y las cookies del motor. Se la pide al
 * programa (`contact_photo`), que la baja una vez, la guarda en la caché y
 * contesta con una `data:` URI. La CSP tampoco deja cargar imágenes de otro
 * origen, así que aunque algo se equivocara acá, el motor no la pediría.
 *
 * Aparte del componente para probarlo sin Tauri: se le pasa el `invoke`.
 */

export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/**
 * Un cargador con memoria: cada dirección se le pide al programa **una vez
 * por sesión** mientras salga bien, y dos fichas que se abren a la vez
 * esperan el mismo pedido.
 *
 * Lo que falla **no** se memoriza: la próxima vez que se abra la ficha se
 * vuelve a preguntar, y el programa contesta desde su marca de fallo sin salir
 * a la red hasta que pase el rato. Memorizarlo acá haría que una foto que se
 * cayó un momento no apareciera hasta cerrar la ventana.
 */
export function createPhotoLoader(invoke: Invoke) {
	const known = new Map<string, Promise<string>>();

	return function loadPhoto(url: string): Promise<string> {
		if (!url) {
			return Promise.resolve('');
		}
		const pending = known.get(url);
		if (pending) {
			return pending;
		}
		const request = invoke<string | null>('contact_photo', { url })
			// Sólo una imagen embebida: cualquier otra cosa que llegara —una
			// dirección, un `javascript:`— no va a un `src`.
			.then((answer) =>
				typeof answer === 'string' && answer.startsWith('data:image/') ? answer : ''
			)
			.catch(() => '')
			.then((uri) => {
				if (!uri) {
					known.delete(url);
				}
				return uri;
			});
		known.set(url, request);
		return request;
	};
}
