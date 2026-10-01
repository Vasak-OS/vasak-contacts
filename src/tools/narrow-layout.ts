/**
 * Una columna por vez en una ventana angosta, como una aplicación de teléfono.
 *
 * Con la ventana ancha se ven las tres columnas —cuentas, lista y ficha— una al
 * lado de la otra, como siempre. Por debajo de `NARROW_ROW` **de fila** (no de
 * pantalla: la barra puede ir a un costado y comerse 48 px) las tres no entran
 * sin aplastarse, y se va de una a otra: cuentas → lista → ficha, con un botón
 * para volver en cada una.
 *
 * Lo decide una consulta de contenedor sobre la fila de la ventana
 * (`@container/row` en `WindowAppLayout`), no un punto de corte de pantalla:
 * WebKitGTK no avisa de `resize` ni de `matchMedia`, y la fila sabe cuánto
 * lugar tiene aunque la ventana no lo sepa. Por eso el estado (`Pane`) se lleva
 * siempre, también con la ventana ancha, donde no se nota: si la persona la
 * angosta, ve la columna en la que estaba.
 *
 * 36 rem son 576 px: la ventana de 600 tiene 590 de fila y se ve igual que
 * hoy, con las tres columnas.
 */
export type Pane = 'accounts' | 'list' | 'detail';

/** Con la ventana angosta, la columna que se ve ocupa toda la fila. */
export const PANE_SHOWN =
	'@max-[36rem]/row:w-full @max-[36rem]/row:max-w-none @max-[36rem]/row:flex-1';

/** Con la ventana angosta, las otras dos no se ven. */
export const PANE_HIDDEN = '@max-[36rem]/row:hidden';

/** Lo que sólo existe con la ventana angosta: los botones de ir y volver. */
export const NARROW_ONLY = '@min-[36rem]/row:hidden';

/** Las clases de una columna según cuál se está mirando. */
export function paneClass(pane: Pane, current: Pane): string {
	return pane === current ? PANE_SHOWN : PANE_HIDDEN;
}

/**
 * Con la ventana angosta, lo que se cortaba con «…» se parte en renglones: una
 * columna sola tiene todo el ancho, pero un correo largo puede no entrar igual,
 * y cortado no se lee. Desde 36rem de fila, cortado como siempre.
 */
export const NARROW_WRAP = '@max-[36rem]/row:whitespace-normal @max-[36rem]/row:wrap-anywhere';

/** Con la ventana angosta, la fila de un dato pone los botones abajo del valor. */
export const NARROW_ROW_WRAP = '@max-[36rem]/row:flex-wrap';

/** El valor de esa fila, que ocupa el renglón entero. */
export const NARROW_FULL_LINE = '@max-[36rem]/row:basis-full';

/** 36rem en píxeles, con la letra raíz del documento, como la mide la consulta. */
export function narrowRowPx(): number {
	const root = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
	return 36 * (Number.isFinite(root) && root > 0 ? root : 16);
}
