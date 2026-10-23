/**
 * Recordar el tema entre arranques para que la ventana no parpadee.
 *
 * El gestor de configuración aplica el esquema del usuario —las variables `--*`
 * sobre el elemento raíz y la clase `.dark`— recién después de montar, así que la
 * ventana abría con el esquema por defecto de la hoja de estilos y viraba al del
 * usuario con la transición de color de 300 ms. Para evitarlo, se guarda el tema
 * ya aplicado y un script en el `<head>` de `index.html` lo reaplica **antes** de
 * montar, de modo que el primer dibujo ya sea el correcto.
 *
 * Este módulo existe aparte para poder probar la lógica; el script del `<head>`
 * es una copia mínima de [`aplicarTema`] porque corre antes del bundle y no puede
 * importar nada.
 */

/** La clave bajo la que vive el tema recordado. La comparte con `index.html`. */
export const CLAVE_TEMA = 'vsk-theme';

/** El tema recordado: la clase oscura y las variables que puso el gestor. */
export interface TemaCacheado {
	dark: boolean;
	vars: Record<string, string>;
}

/**
 * Guarda el tema que el gestor de configuración dejó aplicado sobre `raiz`.
 *
 * Lee las variables `--*` que el gestor escribió como estilo en línea (usa
 * `setProperty` sobre el elemento raíz) y si está la clase `.dark`. No lanza: el
 * afinado pre-boot es una mejora, no una necesidad, así que un `localStorage`
 * inaccesible (modo privado, bloqueado) no debe romper el arranque.
 */
export function guardarTema(
	raiz: HTMLElement = document.documentElement,
	almacen: Storage = localStorage
): void {
	try {
		const vars: Record<string, string> = {};
		for (const nombre of Array.from(raiz.style)) {
			if (nombre.startsWith('--')) vars[nombre] = raiz.style.getPropertyValue(nombre);
		}
		const tema: TemaCacheado = { dark: raiz.classList.contains('dark'), vars };
		almacen.setItem(CLAVE_TEMA, JSON.stringify(tema));
	} catch {
		// localStorage puede no estar disponible: sin recuerdo, arranca como antes.
	}
}

/**
 * Reaplica sobre `raiz` el tema recordado. La usa la prueba; el arranque real lo
 * hace la copia mínima del `<head>` de `index.html`, que corre antes del bundle.
 */
export function aplicarTema(
	raiz: HTMLElement = document.documentElement,
	almacen: Storage = localStorage
): void {
	try {
		const crudo = almacen.getItem(CLAVE_TEMA);
		if (!crudo) return;
		const tema = JSON.parse(crudo) as TemaCacheado;
		if (tema.dark) raiz.classList.add('dark');
		for (const nombre in tema.vars) raiz.style.setProperty(nombre, tema.vars[nombre]);
	} catch {
		// Sin recuerdo válido, arranca como antes.
	}
}
