import { getCurrentWindow } from '@tauri-apps/api/window';
import { useConfigStore } from '@vasakgroup/plugin-config-manager';
import { getIconSource } from '@vasakgroup/plugin-vicons';
import { setupContextMenu } from '@vasakgroup/plugin-vsk-contextual-menu';
import { captureFailures } from '@vasakgroup/plugin-vsk-journal';
import I18n from '@vasakgroup/tauri-plugin-i18n';
import { createPinia } from 'pinia';
import { createApp } from 'vue';
import App from '@/App.vue';
import { esperarArranque } from '@/tools/arranque';
import { sanearUrl } from '@/tools/csp';
import '@/assets/main.css';

/**
 * Cuánto se espera a los textos y al tema antes de abrir la ventana.
 *
 * Se espera para que la primera pantalla no muestre las claves crudas ni el tema
 * equivocado, pero con un plazo: si el backend no contesta, es mejor una ventana
 * fea que ninguna. Ver `tools/arranque.ts`.
 *
 * Tres segundos es mucho más de lo que tardan las dos lecturas —son dos llamadas
 * al proceso de Rust, que ya tiene los datos en memoria— y por eso vencer el
 * plazo se registra: no es el caso normal, es que algo está mal.
 */
const PLAZO_ARRANQUE_MS = 3000;

// Una violación de CSP no se ve: el recurso no carga y la interfaz queda a
// medias sin decir nada. Esto la manda a la consola, saneada.
document.addEventListener('securitypolicyviolation', (evento) => {
	// El respaldo va **después** de sanear, no antes.
	//
	// Mirando el valor crudo, una entrada como `?token=X` es verdadera y pasa
	// el respaldo de largo — pero lo que queda de ella al sanearla es nada, así
	// que el registro salía con el campo en blanco. Sanear primero y decidir
	// después es lo que hace que un aviso incompleto no exista.
	const recurso = sanearUrl(evento.blockedURI) || '(en línea)';
	const origen = sanearUrl(evento.sourceFile) || 'documento';
	console.error(
		`[CSP] bloqueado ${recurso} por la directiva ` +
			`«${evento.violatedDirective}» en ${origen}:${evento.lineNumber}`
	);
});

// Lo que rompe la interfaz va al diario del sistema, con el nombre de esta
// aplicación. Antes no iba a ninguna parte: un error de JavaScript deja la
// pantalla a medias y no queda registro de por qué, y la consola del WebView no
// la ve nadie en una máquina instalada.
//
// Va lo más arriba posible, antes de armar la aplicación, para que también
// atrape lo que falle durante el arranque.
captureFailures();

// El clic derecho abre el menú de VasakOS —el mismo de todo el escritorio— y no
// el del motor del navegador, que ofrece «Recargar» e «Inspeccionar elemento».
setupContextMenu({ iconResolver: getIconSource });

const app = createApp(App);
const pinia = createPinia();

app.use(pinia);

// Un error de Vue en producción no va a ninguna parte; al menos que quede en la
// consola con el contexto de dónde ocurrió.
app.config.errorHandler = (error, _instancia, info) => {
	console.error(`[vue] falló en ${info}:`, error);
};

// Las dos cosas que la ventana necesita tener antes del primer dibujo.
//
// Los textos, una sola vez para toda la aplicación: el `onMounted` de cada
// `useI18n()` carga sólo si falta, así que cargando antes de montar el primero
// que mira ya lo encuentra hecho.
//
// Acá hubo un rodeo hasta la 2.3.0 del plugin: se llamaba a `useI18n().reload()`
// porque la clase y el composable guardaban el catálogo por separado y el `t()`
// de los componentes leía el del composable, así que esperar a la clase no
// adelantaba nada. Ahora hay un solo estado y `load()` alcanza — de ahí que el
// `package.json` pida `^2.3.0` y no `^2.1.0`: con la vieja, esto se monta con
// las claves a la vista.
//
// El tema venía del `onMounted` de `App.vue`, o sea después del primer dibujo:
// hasta que resolvía, las variables `--use-*` no existían y el `@theme` de la
// hoja de estilo caía en los valores por omisión, que son los del tema claro.
// Acá se lee antes de montar, y `App.vue` se queda sólo con el aviso de cambios.
const arranqueCompleto = await esperarArranque(
	[I18n.getInstance().load(), useConfigStore().loadConfig()],
	PLAZO_ARRANQUE_MS
);

if (!arranqueCompleto) {
	// Y si después de abrir la ventana alguna de las dos termina, se ve el
	// repintado: las claves cambiando a texto, o el tema acomodándose. Es feo y es
	// a propósito — la alternativa sería no mostrar la ventana hasta que terminen,
	// que es justo lo que el plazo existe para evitar. Un repintado tardío después
	// de tres segundos de falla es mejor que una aplicación que no abre.
	console.error(
		`El arranque no terminó en ${PLAZO_ARRANQUE_MS} ms: la ventana se abre con ` +
			'lo que haya, y puede mostrar las claves sin traducir o el tema por omisión.'
	);
}

app.mount('#app');

// Y recién ahora se muestra.
//
// La ventana nace oculta (`"visible": false` en `tauri.conf.json`) porque no hay
// forma de que el primer dibujo salga bien: el motor pinta el documento antes de
// que ningún JavaScript haya corrido, así que cualquier ventana visible desde el
// principio enseña el fondo por omisión. Ocultarla mueve el problema a que la
// ventana tarde en aparecer, que es lo que el plazo de arriba acota — y si el
// JavaScript nunca llega a esta línea, la muestra el proceso de Rust. Ver
// `src-tauri/src/ventana.rs`.
try {
	await getCurrentWindow().show();
} catch (error) {
	console.error('No se pudo mostrar la ventana', error);
}
