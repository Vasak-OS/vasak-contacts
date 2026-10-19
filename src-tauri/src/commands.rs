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

/// El recurso del almacén que cubre los contactos. Es el mismo nombre que usa
/// `vasak-accounts` (`sync/src/access.rs`: `CONTACTS_RESOURCE`) y con el que
/// `vasak-permissions` guarda la decisión: tiene que coincidir letra por letra,
/// o volver a pedirlo reabriría el diálogo de otra cosa —o de nada—.
const CONTACTS_RESOURCE: &str = "store.contacts";

/// Lo que devuelve [`contactos_de_la_cuenta`] cuando la persona le dijo que no
/// al almacén. **No se traduce**: es una marca que mira la ventana para pintar
/// el estado de «sin permiso» con su botón, y el texto lo pone ella en el
/// idioma de la sesión (`src/tools/store-permission.ts`). Una frase traducida
/// acá saldría en el idioma con el que se compiló el programa, no en el de quien
/// mira, y obligaría a la ventana a reconocerla por su contenido.
pub(crate) const STORE_PERMISSION_DENIED: &str = "store-permission-denied";

/// El servicio de permisos, en el **bus de sistema** (`vasak-permissions`).
const PERMISSIONS_SERVICE: &str = "ar.net.vasak.os.Permissions";
const PERMISSIONS_PATH: &str = "/ar/net/vasak/os/Permissions";
const PERMISSIONS_INTERFACE: &str = "ar.net.vasak.os.Permissions";
/// Vuelve a abrir el diálogo del permiso de quien llama. Ver `vasak-permissions`
/// (`vasak_permissions_protocol::REQUEST_AGAIN_METHOD`).
const REQUEST_AGAIN_METHOD: &str = "RequestAgain";

/// Las marcas de error de [`request_store_permission_again`], que la ventana
/// traduce. No se traducen acá por lo mismo que [`STORE_PERMISSION_DENIED`].
pub(crate) const REASK_LIMIT: &str = "reask-limit-exceeded";
pub(crate) const REASK_DENIED: &str = "reask-access-denied";
pub(crate) const REASK_FAILED: &str = "reask-failed";

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
        Err(e) if denied_by_store(&e) => return Err(STORE_PERMISSION_DENIED.into()),
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

/// La dirección de la tarjeta propia que dice el servidor de la cuenta
/// (`CARDDAV:principal-address`), o `None` si no la dice. Sólo para **proponer**
/// cuál es la tarjeta de la persona: la elección final es suya.
///
/// Va al servidor, así que pide la credencial de la cuenta (el diálogo de
/// permiso, la primera vez); por eso la ventana lo llama sólo cuando la persona
/// pide proponer su tarjeta y trata cualquier error como «sin propuesta». Es de
/// sólo lectura.
#[tauri::command]
pub async fn own_card_address(account_id: String) -> Result<Option<String>, String> {
    let credencial = accounts::credencial_de(&account_id).await?;
    carddav::direccion_propia(&credencial).await
}

/// Crea o edita un contacto en el almacén del sistema.
///
/// `payload` es el parche que arma la ventana (`src/tools/contact-patch.ts`):
/// ausente no toca, `null` saca, un valor fija. `if_match` vacío **crea** (el
/// parche no lleva `id`); con un ETag **edita** el contacto cuyo `id` —el número
/// local— va en el parche. Devuelve el número local, con el que la ventana puede
/// después borrarlo sin releer.
///
/// Un «no» de la persona vuelve como [`STORE_PERMISSION_DENIED`], la misma marca
/// que la lectura, para que la ventana muestre un texto en el idioma de la
/// sesión en vez de un error crudo del bus.
#[tauri::command]
pub async fn save_contact(
    app: tauri::AppHandle,
    account_id: String,
    address_book_id: String,
    payload: serde_json::Value,
    if_match: String,
) -> Result<String, String> {
    use tauri_plugin_vsk_store::StoreExt;
    let payload = serde_json::to_string(&payload)
        .map_err(|e| format!("no se pudo armar el contacto: {e}"))?;
    crate::store::put_contact(
        &app.store(),
        &account_id,
        &address_book_id,
        &payload,
        &if_match,
    )
    .await
    .map_err(denied_or)
}

/// Borra un contacto del almacén del sistema. `contact_id` es el número local
/// (el `store_id` del contacto). Un «no» vuelve como [`STORE_PERMISSION_DENIED`],
/// como en [`save_contact`].
#[tauri::command]
pub async fn delete_contact(
    app: tauri::AppHandle,
    account_id: String,
    contact_id: String,
    if_match: String,
) -> Result<(), String> {
    use tauri_plugin_vsk_store::StoreExt;
    crate::store::delete_contact(&app.store(), &account_id, &contact_id, &if_match)
        .await
        .map_err(denied_or)
}

/// Los contactos en conflicto de una cuenta: un cambio local que chocó con el
/// servidor. El área es siempre `contacts`. Un «no» vuelve como
/// [`STORE_PERMISSION_DENIED`], como en las lecturas.
#[tauri::command]
pub async fn list_contact_conflicts(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<Vec<crate::store::ConflictItem>, String> {
    use tauri_plugin_vsk_store::StoreExt;
    crate::store::list_conflicts(&app.store(), &account_id, crate::store::CONTACTS_AREA)
        .await
        .map_err(denied_or)
}

/// La comparación de un contacto en conflicto —la versión local y la del
/// servidor—, por su número local (`store_id`). Para la vista «mantener la mía /
/// tomar la del servidor». Un «no» vuelve como [`STORE_PERMISSION_DENIED`].
#[tauri::command]
pub async fn get_contact_conflict(
    app: tauri::AppHandle,
    account_id: String,
    id: String,
) -> Result<crate::store::ConflictDetail, String> {
    use tauri_plugin_vsk_store::StoreExt;
    crate::store::get_conflict(&app.store(), &account_id, crate::store::CONTACTS_AREA, &id)
        .await
        .map_err(denied_or)
}

/// Resuelve un conflicto de un contacto: `keep_local` deja el cambio local,
/// `take_server` toma la versión del servidor. El área es siempre `contacts`. Un
/// «no» vuelve como [`STORE_PERMISSION_DENIED`].
#[tauri::command]
pub async fn resolve_contact_conflict(
    app: tauri::AppHandle,
    account_id: String,
    id: String,
    choice: crate::store::ConflictChoice,
) -> Result<(), String> {
    use tauri_plugin_vsk_store::StoreExt;
    crate::store::resolve_conflict(
        &app.store(),
        &account_id,
        crate::store::CONTACTS_AREA,
        &id,
        choice,
    )
    .await
    .map_err(denied_or)
}

/// Pasa un «no» de la persona a la marca que la ventana traduce; cualquier otro
/// error viaja como vino, para que se vea el motivo.
fn denied_or(error: String) -> String {
    if denied_by_store(&error) {
        STORE_PERMISSION_DENIED.to_string()
    } else {
        error
    }
}

/// Si el almacén contestó que la persona no dio permiso. Ahí no se va al
/// servidor: eso pediría un permiso más fuerte que el que acaba de negar.
pub(crate) fn denied_by_store(error: &str) -> bool {
    error.contains("org.freedesktop.DBus.Error.AccessDenied")
}

/// Vuelve a pedir el permiso para leer los contactos del almacén.
///
/// Llama a `RequestAgain` de `vasak-permissions` en el **bus de sistema**. Ese
/// método olvida **sólo** la decisión de esta ventana sobre `store.contacts`
/// —nunca concede—, así que el próximo intento de leer vuelve a abrir el
/// diálogo. Quién llama lo decide el servicio por la conexión, no un parámetro:
/// esta ventana sólo puede re-pedirse el permiso **a sí misma**, y hay un techo
/// de uno por minuto contra la fatiga de permisos.
///
/// Devuelve una marca que la ventana traduce: [`REASK_LIMIT`] si se pidió hace
/// muy poco, [`REASK_DENIED`] si el recurso no es suyo, y [`REASK_FAILED`] para
/// lo demás (el servicio caído, el bus cerrado).
#[tauri::command]
pub async fn request_store_permission_again() -> Result<(), String> {
    let connection = zbus::Connection::system().await.map_err(|e| {
        eprintln!(
            "[contactos] no se pudo abrir el bus del sistema para volver a pedir el permiso: {e}"
        );
        REASK_FAILED.to_string()
    })?;
    connection
        .call_method(
            Some(PERMISSIONS_SERVICE),
            PERMISSIONS_PATH,
            Some(PERMISSIONS_INTERFACE),
            REQUEST_AGAIN_METHOD,
            &(CONTACTS_RESOURCE,),
        )
        .await
        .map(|_| ())
        .map_err(|e| {
            let code = reask_error_code(method_error_name(&e));
            // Lo esperado —el techo y el fuera de alcance— ya lo dice la ventana;
            // lo demás se anota, que es un fallo de verdad del servicio o del bus.
            if code == REASK_FAILED {
                eprintln!("[contactos] no se pudo volver a pedir el permiso del almacén: {e}");
            }
            code.to_string()
        })
}

/// El nombre del error de D-Bus de un error de `zbus`, si lo lleva. Un error
/// que no viene de un método (el bus cerrado, un tiempo agotado) no tiene
/// nombre y cae en [`REASK_FAILED`].
fn method_error_name(error: &zbus::Error) -> Option<&str> {
    match error {
        zbus::Error::MethodError(name, _, _) => Some(name.as_str()),
        _ => None,
    }
}

/// La marca para la ventana a partir del nombre del error de D-Bus de
/// `RequestAgain`. Pura y sobre el nombre —no sobre el `zbus::Error`, que lleva
/// un `Message` imposible de armar en una prueba—, para poder probar el mapeo
/// sin un bus.
pub(crate) fn reask_error_code(error_name: Option<&str>) -> &'static str {
    match error_name {
        Some("org.freedesktop.DBus.Error.LimitsExceeded") => REASK_LIMIT,
        Some("org.freedesktop.DBus.Error.AccessDenied") => REASK_DENIED,
        _ => REASK_FAILED,
    }
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

    /// Cada error de `RequestAgain` se traduce a la marca que la ventana sabe
    /// pintar: el techo y el fuera de alcance tienen su mensaje; lo demás cae en
    /// el genérico, nunca en algo crudo.
    #[test]
    fn el_error_de_volver_a_pedir_se_mapea_a_su_marca() {
        assert_eq!(
            reask_error_code(Some("org.freedesktop.DBus.Error.LimitsExceeded")),
            REASK_LIMIT
        );
        assert_eq!(
            reask_error_code(Some("org.freedesktop.DBus.Error.AccessDenied")),
            REASK_DENIED
        );
        // Un error con otro nombre no es ni el techo ni el fuera de alcance.
        assert_eq!(
            reask_error_code(Some("org.freedesktop.DBus.Error.ServiceUnknown")),
            REASK_FAILED
        );
        // Y un error que no viene de un método —el bus cerrado— tampoco tiene
        // nombre que mirar.
        assert_eq!(reask_error_code(None), REASK_FAILED);
    }
}
