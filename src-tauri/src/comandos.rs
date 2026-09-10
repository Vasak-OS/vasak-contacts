//! Lo que la ventana le puede pedir al programa.
//!
//! ── Por qué se trae toda la libreta de una ──────────────────────────────────
//!
//! Una agenda se **busca**, y para buscar hay que tener todo. Traer los mil
//! contactos una vez al abrir cuesta unos pocos megabytes y hace que escribir
//! en el buscador sea instantáneo; pedirle al servidor con cada tecla sería
//! lento para la persona y maleducado con el servidor.

use serde::Serialize;

use crate::carddav;
use crate::cuentas::{self, CuentaConLibreta};
use crate::vcard::Contacto;

/// Lo que se pudo leer de una cuenta, y lo que no.
///
/// Las dos cosas juntas, como en el calendario y por lo mismo: una libreta
/// vacía y una que no se pudo leer se ven idénticas, y la diferencia importa.
/// «No tengo a nadie anotado» y «no sé a quién tengo anotado» no son lo mismo.
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct LecturaDeCuenta {
    pub libretas: Vec<carddav::Libreta>,
    pub contactos: Vec<Contacto>,
    /// Las libretas que no se pudieron leer, con el motivo. Vacío si salió todo
    /// bien.
    pub fallos: Vec<String>,
}

/// Las cuentas conectadas que tienen libretas.
///
/// No pide permiso: son metadatos y el servicio ya acota lo que devuelve a
/// quien pregunta. El diálogo aparece al leer los contactos, que es cuando hace
/// falta la credencial.
#[tauri::command]
pub async fn listar_cuentas() -> Result<Vec<CuentaConLibreta>, String> {
    cuentas::cuentas().await
}

/// Todos los contactos de una cuenta.
#[tauri::command]
pub async fn contactos_de_la_cuenta(account_id: String) -> Result<LecturaDeCuenta, String> {
    let credencial = cuentas::credencial_de(&account_id).await?;
    let libretas = carddav::libretas(&credencial).await?;

    let mut lectura = LecturaDeCuenta {
        libretas: libretas.clone(),
        ..Default::default()
    };

    for libreta in &libretas {
        match carddav::contactos(&credencial, &libreta.url).await {
            Ok(contactos) => lectura.contactos.extend(contactos),
            // Nombre y motivo: «falló una libreta» no le dice a nadie cuál de
            // las suyas le falta.
            Err(e) => lectura.fallos.push(format!("{}: {e}", libreta.nombre)),
        }
    }

    // **El orden lo pone la ventana, no esto.**
    //
    // Comparar cadenas por sus bytes manda «Álvarez» después de «Zaparte»,
    // porque la Á ocupa dos bytes que empiezan por encima de cualquier letra
    // ASCII. En español eso desordena media agenda, y pasar todo a minúsculas
    // no lo arregla: el problema es el acento, no la mayúscula.
    //
    // Ordenar bien pide una tabla de intercalación por idioma, y la ventana
    // tiene una: `Intl.Collator` con el idioma de la sesión. Hacerlo acá a
    // medias sería un orden equivocado que la ventana tendría que rehacer.
    Ok(lectura)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcard::Contacto;

    fn contacto(orden: &str) -> Contacto {
        Contacto {
            orden: orden.into(),
            nombre: orden.into(),
            ..Default::default()
        }
    }

    /// Cada contacto sale con **con qué ordenarlo**, aunque el orden lo ponga
    /// la ventana. Sin el campo, ordenar por lo que se muestra pondría a todas
    /// las Anas juntas y a los Pérez desparramados.
    #[test]
    fn cada_contacto_sale_con_su_clave_de_orden() {
        let lectura = LecturaDeCuenta {
            contactos: vec![contacto("Álvarez, Ana")],
            ..Default::default()
        };
        let json = serde_json::to_value(&lectura).unwrap();
        assert_eq!(json["contactos"][0]["orden"], "Álvarez, Ana");
    }

    /// Una libreta rota no puede vaciar la agenda: la persona tiene que ver las
    /// que sí andan y enterarse de cuál le falta.
    #[test]
    fn una_lectura_puede_traer_contactos_y_fallos_a_la_vez() {
        let lectura = LecturaDeCuenta {
            libretas: vec![carddav::Libreta {
                url: "https://x/a/".into(),
                nombre: "Personal".into(),
            }],
            contactos: vec![contacto("Pérez, Ana")],
            fallos: vec!["Trabajo: el servidor respondió 500".into()],
        };

        let json = serde_json::to_value(&lectura).unwrap();
        assert_eq!(json["contactos"].as_array().unwrap().len(), 1);
        // El nombre de la libreta que falló va en el mensaje: «falló una
        // libreta» no le dice a nadie cuál de las suyas le falta.
        assert!(json["fallos"][0].as_str().unwrap().starts_with("Trabajo:"));
    }
}
