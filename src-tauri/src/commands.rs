//! Lo que la ventana le puede pedir al programa.
//!
//! ── Por qué se trae toda la libreta de una ──────────────────────────────────
//!
//! Una agenda se **busca**, y para buscar hay que tener todo. Traer los mil
//! contactos una vez al abrir cuesta unos pocos megabytes y hace que escribir
//! en el buscador sea instantáneo; pedirle al servidor con cada tecla sería
//! lento para la persona y maleducado con el servidor.

use serde::Serialize;

use crate::accounts::{self, CuentaConLibreta};
use crate::carddav;
use crate::vcard::Contact;

/// Lo que se pudo leer de una cuenta, y lo que no.
///
/// Las dos cosas juntas, como en el calendario y por lo mismo: una libreta
/// vacía y una que no se pudo leer se ven idénticas, y la diferencia importa.
/// «No tengo a nadie anotado» y «no sé a quién tengo anotado» no son lo mismo.
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct LecturaDeCuenta {
    pub libretas: Vec<carddav::Libreta>,
    pub contactos: Vec<Contact>,
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
    accounts::cuentas().await
}

/// Los contactos de una cuenta.
///
/// **Primero del almacén del sistema**, que ya está sincronizado y no espera a
/// la red. Sólo si el almacén todavía no tiene libretas de esa cuenta —o no se
/// lo pudo leer— se le pregunta al servidor como antes.
#[tauri::command]
pub async fn contactos_de_la_cuenta(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<LecturaDeCuenta, String> {
    use tauri_plugin_vsk_store::StoreExt;
    match crate::store::read_contacts(&app.store(), &account_id).await {
        Ok(Some(lectura)) => return Ok(lectura),
        Ok(None) => {}
        // Si la persona le dijo que no al almacén, no se le vuelve a preguntar
        // por la puerta de al lado: ir al servidor pediría `account.contacts`,
        // que entrega el token y es más que lo que acaba de negar.
        Err(e) if denied_by_store(&e) => {
            return Err(
                "no diste permiso para leer los contactos del sistema; se cambia en \
                 Configuración → Privacidad y seguridad"
                    .into(),
            )
        }
        Err(e) => eprintln!("[contactos] el almacén no contestó, se va al servidor: {e}"),
    }
    let credencial = accounts::credencial_de(&account_id).await?;
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

/// Si el almacén contestó que la persona no dio permiso. Ahí no se va al
/// servidor: eso pediría un permiso más fuerte que el que acaba de negar.
pub(crate) fn denied_by_store(error: &str) -> bool {
    error.contains("org.freedesktop.DBus.Error.AccessDenied")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcard::Contact;

    fn contact(sort_key: &str) -> Contact {
        Contact {
            sort_key: sort_key.into(),
            name: sort_key.into(),
            ..Default::default()
        }
    }

    /// Cada contacto sale con **con qué ordenarlo**, aunque el orden lo ponga
    /// la ventana. Sin el campo, ordenar por lo que se muestra pondría a todas
    /// las Anas juntas y a los Pérez desparramados.
    #[test]
    fn cada_contacto_sale_con_su_clave_de_orden() {
        let lectura = LecturaDeCuenta {
            contactos: vec![contact("Álvarez, Ana")],
            ..Default::default()
        };
        let json = serde_json::to_value(&lectura).unwrap();
        assert_eq!(json["contactos"][0]["sort_key"], "Álvarez, Ana");
    }

    /// La libreta de cada contacto viaja con el nombre que lee la ventana
    /// (`src/tools/address-book.ts`): Tauri no convierte las claves, así que si
    /// cambia el nombre del campo, el filtro por libreta (#4) deja de ver nada.
    #[test]
    fn cada_contacto_sale_con_su_libreta() {
        let lectura = LecturaDeCuenta {
            contactos: vec![Contact {
                sort_key: "Pérez, Ana".into(),
                address_book_id: "personal".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let json = serde_json::to_value(&lectura).unwrap();
        assert_eq!(json["contactos"][0]["address_book_id"], "personal");
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
            contactos: vec![contact("Pérez, Ana")],
            fallos: vec!["Trabajo: el servidor respondió 500".into()],
        };

        let json = serde_json::to_value(&lectura).unwrap();
        assert_eq!(json["contactos"].as_array().unwrap().len(), 1);
        // El nombre de la libreta que falló va en el mensaje: «falló una
        // libreta» no le dice a nadie cuál de las suyas le falta.
        assert!(json["fallos"][0].as_str().unwrap().starts_with("Trabajo:"));
    }

    /// Un «no» al almacén no se convierte en una pregunta por el permiso más
    /// fuerte de la cuenta; cualquier otro fallo del almacén sí va al servidor.
    #[test]
    fn un_no_del_almacen_no_lleva_al_servidor() {
        assert!(denied_by_store(
            "ListAddressBooks: org.freedesktop.DBus.Error.AccessDenied: sin permiso"
        ));
        assert!(!denied_by_store(
            "ListAddressBooks: org.freedesktop.DBus.Error.ServiceUnknown: no está"
        ));
        assert!(!denied_by_store("respuesta inválida de ListContactCards"));
    }
}
