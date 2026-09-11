/**
 * Lo que tiene que estar listo antes de dibujar la ventana por primera vez.
 *
 * Al arrancar hay dos cosas que llegan tarde y se ven: los textos —hasta que el
 * catálogo de idioma está cargado, la interfaz muestra las claves crudas,
 * «casillas.entrada» donde tendría que decir «Entrada»— y el tema, que hasta que
 * el gestor de configuración escribe las variables CSS cae en los valores por
 * omisión de la hoja de estilo, que son los del tema claro. En un escritorio
 * oscuro eso es un fogonazo blanco.
 *
 * La solución es esperar a las dos antes de montar, y que la ventana ni siquiera
 * esté en pantalla mientras tanto. Pero esperar sin plazo es peor que el
 * destello: si el backend no contesta, la aplicación no abre nunca y no dice por
 * qué. De ahí este módulo.
 *
 * Va aparte de `main.ts` para poder probarlo: importar `main.ts` arranca la
 * aplicación entera.
 */

/**
 * Espera a que termine el arranque, pero no para siempre.
 *
 * Devuelve `true` si todas las tareas terminaron dentro del plazo y `false` si
 * se agotó. Quien llama decide qué hacer con eso —acá se abre la ventana igual,
 * porque una ventana fea es mejor que ninguna—, pero conviene que quede dicho en
 * algún registro: que el plazo venza significa que algo del backend está mal.
 *
 * Una tarea que **falla** no es un arranque fallido. El error se registra donde
 * corresponda y la ventana se abre con lo que haya: sin traducciones se ven las
 * claves, sin configuración se ve el tema por omisión, y las dos cosas son
 * usables. Lo que no es usable es no abrir. Por eso cada tarea va con su `catch`
 * antes de entrar en el `Promise.all`: sin eso, una sola que rechace descarta la
 * espera de las demás **y** deja un rechazo sin atender.
 */
export function esperarArranque(tareas: Promise<unknown>[], plazoMs: number): Promise<boolean> {
	let reloj: ReturnType<typeof setTimeout> | undefined;

	const vencido = new Promise<boolean>((resolver) => {
		reloj = setTimeout(() => resolver(false), plazoMs);
	});

	const todas = Promise.all(
		tareas.map((tarea, i) =>
			tarea.catch((error) => {
				// Sin esto el fallo no existe en ninguna parte: la ventana abre con
				// el tema por omisión o con las claves a la vista, y no hay nada que
				// mirar para saber por qué. `loadConfig()` es el caso puntual —
				// propaga los rechazos del backend sin registrarlos.
				console.error(`Falló la tarea de arranque n.º ${i}`, error);
			})
		)
	).then(() => true);

	// El `clearTimeout` no es cosmético: sin él, un plazo de varios segundos deja
	// un temporizador vivo después de que la ventana ya se dibujó.
	return Promise.race([todas, vencido]).finally(() => clearTimeout(reloj));
}
