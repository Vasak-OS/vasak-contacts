import { invoke } from '@tauri-apps/api/core';
import { computed, ref } from 'vue';
import { buscados, type Contacto, ordenados } from '@/tools/agenda';

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
	contactos: Contacto[];
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
	const contactos = ref<Contacto[]>([]);
	const elegido = ref<Contacto | null>(null);
	const consulta = ref('');
	const cargando = ref(false);
	/** Lo que impidió leer algo, en el idioma de lo que la persona puede hacer. */
	const avisos = ref<string[]>([]);

	/**
	 * Cuál es la carga vigente.
	 *
	 * Sin esto, dos actualizaciones seguidas dejan dos pedidos en el aire y gana
	 * el que conteste último. En una agenda eso se ve como contactos que
	 * aparecen y desaparecen sin motivo.
	 */
	let vigente = 0;

	/** Ordenados con el comparador del idioma de la sesión. Ver `agenda.ts`. */
	const enOrden = computed(() => ordenados(contactos.value, locale()));
	const visibles = computed(() => buscados(enOrden.value, consulta.value));

	async function cargar() {
		const mio = ++vigente;
		cargando.value = true;
		// Los avisos se juntan acá, al empezar, y se publican al final. Vaciarlos
		// después de traer los datos borraría justo lo que esta carga acaba de
		// descubrir, y el aviso no llegaría a verse nunca.
		const nuevosAvisos: string[] = [];

		try {
			const conectadas = await invoke<Cuenta[]>('listar_cuentas');
			if (mio !== vigente) {
				return;
			}
			cuentas.value = conectadas;

			const juntados: Contacto[] = [];
			for (const cuenta of conectadas) {
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
					juntados.push(...lectura.contactos);
					nuevosAvisos.push(...lectura.fallos);
				} catch (e) {
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

			// Si el que estaba abierto ya no está, se cierra el panel: dejarlo
			// mostraría a alguien que se borró desde otro dispositivo como si
			// siguiera en la agenda.
			if (elegido.value && !juntados.some((c) => c.url === elegido.value?.url)) {
				elegido.value = null;
			}
		} catch (e) {
			if (mio !== vigente) {
				return;
			}
			// Que no esté el servicio de cuentas no es un fallo de esta ventana:
			// simplemente no hay contactos que mostrar.
			cuentas.value = [];
			contactos.value = [];
			elegido.value = null;
			nuevosAvisos.push(String(e));
		} finally {
			if (mio === vigente) {
				avisos.value = nuevosAvisos;
				cargando.value = false;
			}
		}
	}

	function elegir(contacto: Contacto) {
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
		cargar,
		elegir,
	};
}
