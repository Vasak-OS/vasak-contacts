import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import {
	type AccountSyncStatus,
	onStoreChanged,
	onSyncStatus,
	requestSync,
	type SyncRequestFailure,
	syncStatus,
} from '@vasakgroup/plugin-vsk-store';
import { computed, onScopeDispose, ref } from 'vue';
import { type Contact, searchContacts, sortContacts } from '@/tools/address-book';
import {
	type AddressBook,
	collectAddressBooks,
	collectCategories,
	filterByBooks,
	filterByCategories,
	loadHiddenBooks,
	saveHiddenBooks,
} from '@/tools/library-filter';
import { isStorePermissionDenied } from '@/tools/store-permission';

/** Una cuenta conectada que tiene libretas. */
export interface Cuenta {
	id: string;
	nombre: string;
	/**
	 * Si hay que reconectarla desde Configuración.
	 *
	 * Se muestra igual: una cuenta que desaparece de la lista parece una cuenta
	 * que se borró, y la persona no sabría que le falta hacer algo.
	 */
	necesita_reconectarse: boolean;
}

/** Una libreta dentro de una cuenta. */
export interface Libreta {
	url: string;
	nombre: string;
}

interface LecturaDeCuenta {
	libretas: Libreta[];
	contactos: Contact[];
	fallos: string[];
}

/**
 * La agenda: qué cuentas hay, quién está anotado y qué se está buscando.
 *
 * ── Por qué se busca acá y no en el servidor ────────────────────────────────
 *
 * Los contactos se traen todos al abrir. Buscar sobre lo que ya está en memoria
 * es instantáneo y no le pega al servidor con cada tecla; una agenda de mil
 * contactos son unos pocos megabytes.
 */
export function useAgenda(locale: () => string) {
	const cuentas = ref<Cuenta[]>([]);
	const contactos = ref<Contact[]>([]);
	const elegido = ref<Contact | null>(null);
	const consulta = ref('');
	const cargando = ref(false);
	/** Lo que impidió leer algo, en el idioma de lo que la persona puede hacer. */
	const avisos = ref<string[]>([]);
	/**
	 * Si el almacén negó el permiso para leer los contactos. El almacén es uno
	 * solo para todas las cuentas: si lo niega, lo niega para todas y la agenda
	 * queda vacía. Entonces la vista muestra el estado de «sin permiso» con el
	 * botón de volver a pedirlo, en vez de un aviso técnico por cada cuenta.
	 */
	const sinPermiso = ref(false);

	/** Las libretas de todas las cuentas, para elegir cuáles se ven. */
	const addressBooks = ref<AddressBook[]>([]);
	/**
	 * Qué libretas se ocultaron. Es preferencia de quien mira, así que se
	 * recuerda en el navegador de la ventana entre una sesión y la siguiente.
	 * Ver `tools/library-filter.ts`.
	 */
	const hiddenBooks = ref<Set<string>>(loadHiddenBooks());
	/**
	 * Qué categorías se eligieron para filtrar. No se recuerda: es un filtro del
	 * momento, como la búsqueda escrita.
	 */
	const selectedCategories = ref<Set<string>>(new Set());

	/**
	 * Cuál es la carga vigente.
	 *
	 * Sin esto, dos actualizaciones seguidas dejan dos pedidos en el aire y gana
	 * el que conteste último. En una agenda eso se ve como contactos que
	 * aparecen y desaparecen sin motivo.
	 */
	let vigente = 0;

	/**
	 * Lo de las libretas que se ven. La lista de categorías sale de acá y no de
	 * lo ya filtrado por categoría: así marcar una categoría no hace desaparecer
	 * a las demás de la barra.
	 */
	const afterBooks = computed(() => filterByBooks(contactos.value, hiddenBooks.value));
	/** Las categorías que hay para filtrar, de las libretas que se ven. */
	const categories = computed(() => collectCategories(afterBooks.value, locale()));
	const afterCategories = computed(() =>
		filterByCategories(afterBooks.value, selectedCategories.value)
	);
	/** Ordenados con el comparador del idioma de la sesión. Ver `address-book.ts`. */
	const enOrden = computed(() => sortContacts(afterCategories.value, locale()));
	const visibles = computed(() => searchContacts(enOrden.value, consulta.value));

	/** Oculta o vuelve a mostrar una libreta, y recuerda la decisión. */
	function toggleBook(key: string) {
		const next = new Set(hiddenBooks.value);
		if (next.has(key)) {
			next.delete(key);
		} else {
			next.add(key);
		}
		hiddenBooks.value = next;
		saveHiddenBooks(next);
	}

	/** Agrega o saca una categoría del filtro. */
	function toggleCategory(category: string) {
		const next = new Set(selectedCategories.value);
		if (next.has(category)) {
			next.delete(category);
		} else {
			next.add(category);
		}
		selectedCategories.value = next;
	}

	/** Saca el filtro de categorías entero. */
	function clearCategories() {
		selectedCategories.value = new Set();
	}

	async function cargar() {
		const mio = ++vigente;
		cargando.value = true;
		// Los avisos se juntan acá, al empezar, y se publican al final. Vaciarlos
		// después de traer los datos borraría justo lo que esta carga acaba de
		// descubrir, y el aviso no llegaría a verse nunca.
		const nuevosAvisos: string[] = [];
		// Si alguna cuenta chocó con el «no» del almacén. Se decide al final, para
		// que un relee con permiso ya concedido lo apague solo.
		let huboDenegacion = false;

		try {
			const conectadas = await invoke<Cuenta[]>('listar_cuentas');
			if (mio !== vigente) {
				return;
			}
			// Una respuesta que no es lista se toma como ninguna cuenta.
			cuentas.value = Array.isArray(conectadas) ? conectadas : [];

			const juntados: Contact[] = [];
			const librosPorCuenta: {
				accountId: string;
				accountName: string;
				books: Libreta[];
			}[] = [];
			for (const cuenta of cuentas.value) {
				if (cuenta.necesita_reconectarse) {
					// No se intenta: el servicio ya sabe que la credencial no
					// sirve, y pedirla sólo agrega un error técnico encima de un
					// aviso que ya dice qué hacer.
					continue;
				}
				try {
					const lectura = await invoke<LecturaDeCuenta>('contactos_de_la_cuenta', {
						accountId: cuenta.id,
					});
					// De qué cuenta es cada contacto lo pone la ventana: el
					// programa no lo manda, y hace falta para distinguir dos
					// libretas con el mismo id en dos cuentas. Ver `library-filter`.
					for (const contacto of lectura.contactos) {
						juntados.push({ ...contacto, account_id: cuenta.id });
					}
					librosPorCuenta.push({
						accountId: cuenta.id,
						accountName: cuenta.nombre,
						books: Array.isArray(lectura.libretas) ? lectura.libretas : [],
					});
					nuevosAvisos.push(...lectura.fallos);
				} catch (e) {
					// El «no» del almacén no es un aviso por cuenta: es el mismo
					// permiso para todas, así que se marca una vez y la vista
					// muestra el estado de «sin permiso» con su botón.
					if (isStorePermissionDenied(e)) {
						huboDenegacion = true;
						continue;
					}
					// Una cuenta que falla no puede vaciar la agenda de las
					// otras. Con el nombre adelante: «no se pudo leer» no le dice
					// a nadie cuál de sus dos cuentas está rota.
					nuevosAvisos.push(`${cuenta.nombre}: ${e}`);
				}
			}

			if (mio !== vigente) {
				return;
			}
			contactos.value = juntados;
			addressBooks.value = collectAddressBooks(librosPorCuenta);

			// El que estaba abierto se **reemplaza** por su versión nueva, no
			// se deja como estaba: si alguien le cambió el teléfono desde otro
			// dispositivo, la lista mostraría el nuevo y el panel de al lado el
			// viejo, sin nada que explique la diferencia.
			//
			// Y si ya no está, se cierra: dejarlo mostraría a alguien que se
			// borró como si siguiera en la agenda.
			if (elegido.value) {
				elegido.value = juntados.find((c) => c.url === elegido.value?.url) ?? null;
			}
		} catch (e) {
			if (mio !== vigente) {
				return;
			}
			// Que no esté el servicio de cuentas no es un fallo de esta ventana:
			// simplemente no hay contactos que mostrar.
			cuentas.value = [];
			contactos.value = [];
			addressBooks.value = [];
			elegido.value = null;
			nuevosAvisos.push(String(e));
		} finally {
			if (mio === vigente) {
				avisos.value = nuevosAvisos;
				sinPermiso.value = huboDenegacion;
				cargando.value = false;
			}
		}
	}

	/**
	 * Vuelve a pedir el permiso del almacén y, si se pudo pedir, relee: al releer
	 * aparece el diálogo de `vasak-permissions`. Si la persona vuelve a decir que
	 * no, la agenda sigue en el estado de «sin permiso». Devuelve la marca del
	 * error cuando no se pudo **ni pedir** —el techo de uno por minuto, o un
	 * recurso ajeno—, para que la vista la traduzca; `null` cuando sí se pidió.
	 */
	async function pedirPermisoDeNuevo(): Promise<string | null> {
		try {
			await invoke('request_store_permission_again');
		} catch (e) {
			return String(e);
		}
		await cargar();
		return null;
	}

	/**
	 * Crea o edita un contacto en el almacén y relee.
	 *
	 * **Optimista + recarga** (#4): se le pide al almacén que guarde, y se
	 * relee enseguida; lo que el almacén sincronice después llega por su aviso
	 * de cambios (`CHANGED_EVENT`), que ya relee solo. El parche lo arma la
	 * ventana (`tools/contact-patch.ts`): `if_match` vacío crea, con un ETag
	 * edita. Devuelve el error crudo del comando si no se pudo —la vista lo
	 * traduce— o `null` si salió bien.
	 */
	async function saveContact(args: {
		accountId: string;
		addressBookId: string;
		payload: Record<string, unknown>;
		ifMatch: string;
	}): Promise<string | null> {
		try {
			await invoke<string>('save_contact', {
				accountId: args.accountId,
				addressBookId: args.addressBookId,
				payload: args.payload,
				ifMatch: args.ifMatch,
			});
		} catch (e) {
			return String(e);
		}
		await cargar();
		return null;
	}

	/**
	 * Borra un contacto del almacén y relee.
	 *
	 * Se lo saca de la lista en el acto —optimista— para que la ventana no
	 * espere, y se relee: si el borrado no se pudo, al releer vuelve a aparecer,
	 * porque el almacén sigue teniéndolo. Devuelve el error crudo si no se pudo,
	 * o `null` si salió bien.
	 */
	async function deleteContact(contacto: Contact): Promise<string | null> {
		// Sin número local no hay a quién borrar: la lectura vino del servidor o
		// es una sub-tarjeta de un recurso con varias.
		if (!contacto.store_id) {
			return 'no-store-id';
		}
		const antes = contactos.value;
		// Optimista: se saca ya, y si estaba abierto se cierra.
		contactos.value = contactos.value.filter((c) => c.url !== contacto.url);
		if (elegido.value?.url === contacto.url) {
			elegido.value = null;
		}
		try {
			await invoke('delete_contact', {
				accountId: contacto.account_id,
				contactId: contacto.store_id,
				ifMatch: contacto.etag,
			});
		} catch (e) {
			// No se pudo: se deja lo que había hasta que la recarga diga la verdad.
			contactos.value = antes;
			return String(e);
		}
		await cargar();
		return null;
	}

	/** El estado de la sincronización de cada cuenta, del almacén. */
	const syncStatuses = ref<AccountSyncStatus[]>([]);
	/** Las cuentas a las que el último «Actualizar» no les pudo pedir una vuelta. */
	const refreshFailures = ref<SyncRequestFailure[]>([]);

	/**
	 * Lee el estado de la sincronización. Si no se puede, se deja el que había:
	 * no saber el estado no es un error que haya que mostrar encima de la agenda.
	 */
	async function loadSyncStatus() {
		try {
			syncStatuses.value = await syncStatus('contacts');
		} catch (e) {
			console.error('no se pudo leer el estado de la sincronización', e);
		}
	}

	/**
	 * El botón «Actualizar»: le pide al sincronizador que traiga ya lo nuevo de
	 * cada cuenta, y relee lo que hay mientras tanto. Lo que llegue después se
	 * avisa con el aviso de cambios del almacén.
	 */
	async function refresh() {
		// Las que hay que reconectar no: el sincronizador no puede con ellas
		// hasta que la persona lo haga desde Configuración.
		const accountIds = cuentas.value.filter((c) => !c.necesita_reconectarse).map((c) => c.id);
		try {
			refreshFailures.value = await requestSync(accountIds);
		} catch (e) {
			console.error('no se pudo pedir una vuelta al sincronizador', e);
			refreshFailures.value = accountIds.map((account_id) => ({
				account_id,
				reason: 'unavailable' as const,
			}));
		}
		await Promise.all([loadSyncStatus(), cargar()]);
	}

	// Lo nuevo que trae el sincronizador en segundo plano: se relee la agenda.
	// Sin preguntar a intervalos.
	let dejarDeEscuchar: UnlistenFn | null = null;
	let descartado = false;
	onStoreChanged(() => {
		void cargar();
	})
		.then((cancelar) => {
			// La ventana se puede cerrar antes de que esto termine de engancharse.
			if (descartado) {
				cancelar();
				return;
			}
			dejarDeEscuchar = cancelar;
		})
		.catch((e) => {
			// Sin el aviso queda el botón de actualizar: se pierde enterarse solo.
			console.error('no se pudo escuchar al almacén de contactos', e);
		});
	// El estado de la sincronización, cada vez que cambia. Un «se pidió hace un
	// momento» deja de valer en cuanto hay una vuelta en marcha.
	let stopWatchingStatus: UnlistenFn | null = null;
	void loadSyncStatus();
	onSyncStatus(() => {
		refreshFailures.value = refreshFailures.value.filter((f) => f.reason !== 'too_soon');
		void loadSyncStatus();
	})
		.then((cancelar) => {
			if (descartado) {
				cancelar();
				return;
			}
			stopWatchingStatus = cancelar;
		})
		.catch((e) => {
			console.error('no se pudo escuchar el estado de la sincronización', e);
		});

	onScopeDispose(() => {
		descartado = true;
		stopWatchingStatus?.();
		dejarDeEscuchar?.();
	});

	function elegir(contacto: Contact) {
		elegido.value = contacto;
	}

	return {
		cuentas,
		contactos,
		visibles,
		elegido,
		consulta,
		cargando,
		avisos,
		sinPermiso,
		cargar,
		refresh,
		pedirPermisoDeNuevo,
		saveContact,
		deleteContact,
		syncStatuses,
		refreshFailures,
		elegir,
		addressBooks,
		hiddenBooks,
		categories,
		selectedCategories,
		toggleBook,
		toggleCategory,
		clearCategories,
	};
}
