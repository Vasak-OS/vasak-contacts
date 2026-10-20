//! Que el afinado del WebKit siga cableado de punta a punta.
//!
//! El glue de GTK no se puede ejercitar sin un display ni un WebKit reales
//! (por eso vive en `commands/`, excluido de cobertura). Esta prueba no corre
//! GTK: lee el fuente y comprueba que las piezas sigan conectadas —el comando,
//! sus palancas, el registro en `lib.rs` y la llamada desde el frontend—, que
//! es lo que se rompe al mover o renombrar algo.

use std::path::PathBuf;

fn leer(ruta_relativa: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ruta_relativa);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
}

#[test]
fn el_comando_afina_las_palancas_seguras() {
    let webkit = leer("src/commands/webkit.rs");
    assert!(
        webkit.contains("pub fn tune_webview"),
        "falta el comando tune_webview"
    );
    assert!(
        webkit.contains("set_enable_developer_extras(false)"),
        "falta apagar los extras de desarrollo"
    );
    assert!(
        webkit.contains("set_enable_back_forward_navigation_gestures(false)"),
        "falta apagar los gestos de navegación"
    );
    assert!(
        webkit.contains("CacheModel::DocumentViewer"),
        "falta el caché de visor de documento"
    );
}

#[test]
fn el_comando_esta_registrado_en_lib() {
    let lib = leer("src/lib.rs");
    assert!(
        lib.contains("commands::webkit::tune_webview"),
        "tune_webview no está en el generate_handler de lib.rs"
    );
}

#[test]
fn el_frontend_lo_invoca_al_montar() {
    let app = leer("../src/App.vue");
    assert!(
        app.contains("invoke('tune_webview')"),
        "App.vue no invoca tune_webview"
    );
}
