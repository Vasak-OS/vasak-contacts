//! Que la ventana se vea, pase lo que pase.
//!
//! La ventana nace oculta —`"visible": false` en `tauri.conf.json`— y la muestra
//! el frontend cuando ya tiene los textos traducidos y los colores del tema. Es
//! la única manera de que el primer dibujo no salga mal: el motor pinta el
//! documento antes de que ningún JavaScript haya corrido, así que una ventana
//! visible desde el principio enseña sí o sí las claves de traducción crudas y
//! el tema claro. Ver `src/main.ts`.
//!
//! Lo que eso compra en un arranque normal lo paga en el anormal: ata la única
//! ventana del programa a que el JavaScript llegue a ejecutarse. Y hay maneras
//! de que no llegue —un error al cargar el paquete, un recurso que la política
//! de contenido bloquea, una excepción antes de montar—, y en todas ellas el
//! resultado sería una aplicación que arranca, no muestra nada y no dice por
//! qué. Este módulo es la red debajo de eso.

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

/// Cuánto se le da al frontend antes de mostrar la ventana por las malas.
///
/// Más largo que el plazo del frontend (3 s) a propósito: esto no es un camino
/// alternativo, es la red. Mientras el frontend pueda hacerlo él, hay que
/// dejarlo — porque él sabe *cuándo* la ventana está lista y esto sólo sabe
/// contar.
const PLAZO_MS: u64 = 5_000;

/// La etiqueta de la única ventana. Es la que Tauri le pone a una ventana
/// declarada sin `label`, y la misma que nombra `capabilities/default.json`.
const VENTANA: &str = "main";

/// Muestra la ventana si el frontend no llegó a pedirlo.
pub fn mostrar_aunque_el_frontend_falle<R: Runtime>(app: AppHandle<R>) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(PLAZO_MS));

        let Some(ventana) = app.get_webview_window(VENTANA) else {
            // La ventana ya no está: o se cerró, o alguien le cambió la
            // etiqueta. En los dos casos no hay nada que mostrar.
            return;
        };

        // Ante la duda, no se hace nada: `is_visible` falla cuando la ventana se
        // está cerrando, y traerla de vuelta ahí sería peor que el problema que
        // esto arregla.
        if ventana.is_visible().unwrap_or(true) {
            return;
        }

        eprintln!(
            "[ventana] el frontend no la mostró en {PLAZO_MS} ms; se muestra igual. \
             Es muy probable que haya fallado al arrancar: mirar el diario."
        );
        let _ = ventana.show();
    });
}

#[cfg(test)]
mod tests {
    //! El arreglo son tres piezas en tres archivos distintos y ninguna sirve
    //! sola: la ventana oculta en la configuración, el permiso para mostrarla en
    //! las capacidades, y la llamada del frontend. Si se cae cualquiera de las
    //! dos primeras, no falla nada al compilar — la aplicación simplemente
    //! vuelve a destellar, o peor, no abre nunca. Esto las ata.

    use serde_json::Value;

    fn json(texto: &str) -> Value {
        serde_json::from_str(texto).expect("el archivo tiene que ser JSON válido")
    }

    #[test]
    fn la_ventana_nace_oculta() {
        // Si alguien saca esto, la aplicación vuelve a mostrar las claves crudas
        // y el tema claro durante el arranque, que es justo lo que se arregló.
        let conf = json(include_str!("../tauri.conf.json"));
        let ventanas = conf["app"]["windows"]
            .as_array()
            .expect("tiene que haber ventanas declaradas");
        assert_eq!(ventanas.len(), 1, "el módulo asume una sola ventana");
        assert_eq!(
            ventanas[0]["visible"],
            Value::Bool(false),
            "la ventana tiene que nacer oculta: la muestra el frontend cuando está lista"
        );
    }

    #[test]
    fn el_frontend_tiene_permiso_para_mostrarla() {
        // Sin este permiso la llamada de `main.ts` es rechazada, y entonces la
        // ventana sólo aparece cuando vence el plazo de este módulo: cinco
        // segundos de nada después de abrir la aplicación. `core:window:default`
        // **no** lo trae, por eso va escrito.
        let capacidades = json(include_str!("../capabilities/default.json"));
        let permisos = capacidades["permissions"]
            .as_array()
            .expect("tiene que haber permisos");
        assert!(
            permisos.iter().any(|p| p == "core:window:allow-show"),
            "falta «core:window:allow-show»: {permisos:?}"
        );
        let ventanas = capacidades["windows"].as_array().expect("tiene que haber ventanas");
        assert!(
            ventanas.iter().any(|v| v == super::VENTANA),
            "la capacidad tiene que cubrir la ventana «{}»: {ventanas:?}",
            super::VENTANA
        );
    }

    #[test]
    fn la_red_espera_mas_que_el_frontend() {
        // El plazo del frontend está en `src/main.ts` (PLAZO_ARRANQUE_MS). Si
        // esta red se disparara antes, el frontend nunca llegaría a mostrar la
        // ventana él mismo y se vería siempre el arranque a medio hacer.
        let main_ts = include_str!("../../src/main.ts");
        let plazo_frontend: u64 = main_ts
            .lines()
            .find_map(|l| l.strip_prefix("const PLAZO_ARRANQUE_MS = "))
            .and_then(|v| v.trim_end_matches(';').trim().parse().ok())
            .expect("no se encontró PLAZO_ARRANQUE_MS en src/main.ts");
        assert!(
            super::PLAZO_MS > plazo_frontend,
            "la red ({}) tiene que esperar más que el frontend ({plazo_frontend})",
            super::PLAZO_MS
        );
    }
}
