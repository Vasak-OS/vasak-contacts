//! Lo que la ventana le puede pedir al programa.
//!
//! ── Por qué se trae toda la libreta de una ──────────────────────────────────
//!
//! Una agenda se **busca**, y para buscar hay que tener todo. Traer los mil
//! contactos una vez al abrir cuesta unos pocos megabytes y hace que escribir
//! en el buscador sea instantáneo; pedirle al servidor con cada tecla sería
//! lento para la persona y maleducado con el servidor.

use serde::Serialize;

pub mod webkit;

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
    request_again(CONTACTS_RESOURCE).await
}

/// Vuelve a pedir el permiso del historial de correo de la ficha
/// (`store.email.interactions`). Igual que [`request_store_permission_again`],
/// con el mismo techo de uno por minuto; el recurso va fijo en el programa y no
/// lo elige la ventana.
#[tauri::command]
pub async fn request_mail_history_permission_again() -> Result<(), String> {
    request_again(crate::store::MAIL_HISTORY_RESOURCE).await
}

async fn request_again(resource: &'static str) -> Result<(), String> {
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
            &(resource,),
        )
        .await
        .map(|_| ())
        .map_err(|e| {
            let code = reask_error_code(method_error_name(&e));
            // Lo esperado —el techo y el fuera de alcance— ya lo dice la ventana;
            // lo demás se anota, que es un fallo de verdad del servicio o del bus.
            if code == REASK_FAILED {
                eprintln!("[contactos] no se pudo volver a pedir el permiso de {resource}: {e}");
            }
            code.to_string()
        })
}

/// El historial de correo con una dirección: los últimos `limit` mensajes
/// intercambiados con ella en la cuenta, más nuevos primero. **Sin cuerpo**.
///
/// Es de consulta, no se guarda nada: la ficha pregunta al abrirse y no se queda
/// con la respuesta. Pide `store.email.interactions` —no `store.email` ni
/// `account.email`—, así que esta aplicación nunca llega a la credencial ni al
/// resto de la casilla. Un «no» vuelve como [`STORE_PERMISSION_DENIED`], la misma
/// marca que el resto; la ventana decide qué pintar.
#[tauri::command]
pub async fn mail_history(
    app: tauri::AppHandle,
    account_id: String,
    address: String,
    limit: u32,
) -> Result<Vec<crate::store::MailInteraction>, String> {
    use tauri_plugin_vsk_store::StoreExt;
    crate::store::read_mail_history(&app.store(), &account_id, &address, limit)
        .await
        .map_err(denied_or)
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

/// Escribe el CSV exportado en la ruta que **eligió la persona** en el diálogo
/// del sistema (`dialog.save` del lado de la ventana): la ventana nunca inventa
/// una ruta, la trae del diálogo. Se antepone un BOM para que Excel abra el
/// UTF-8 sin romper los acentos.
///
/// La escritura es atómica —a un temporal en el mismo directorio y luego
/// `rename`— para no dejar un archivo a medias si algo se corta, y sigue el
/// enlace simbólico para no reemplazarlo por un archivo común (mismo criterio
/// que `vasak-text`).
#[tauri::command]
pub async fn exportar_csv(ruta: String, csv: String) -> Result<(), String> {
    // En un hilo aparte: Tauri 2 corre los comandos síncronos en el hilo
    // principal, y escribir en un disco lento o de red colgaría la ventana (y la
    // vista no podría ni mostrar que está guardando).
    tauri::async_runtime::spawn_blocking(move || exportar_csv_impl(&ruta, &csv))
        .await
        .map_err(|e| format!("no se pudo guardar el archivo: {e}"))?
}

/// El trabajo de `exportar_csv`, aparte para poder probarlo sin un runtime async.
fn exportar_csv_impl(ruta: &str, csv: &str) -> Result<(), String> {
    let mut contenido = String::from("\u{feff}");
    contenido.push_str(csv);
    escribir_atomico(std::path::Path::new(ruta), contenido.as_bytes())
        .map_err(|e| format!("no se pudo guardar el archivo: {e}"))
}

/// Exporta a vCard las tarjetas de una cuenta en la ruta que **eligió la
/// persona** en el diálogo del sistema. A diferencia del CSV —que la ventana
/// arma desde el modelo de pantalla—, acá se leen las tarjetas **crudas** del
/// almacén y se concatena su texto original: así el `.vcf` conserva las fotos,
/// las direcciones y los cumpleaños que `Contact` descarta, sin reescribir nada.
///
/// `address_book_id` vacío exporta toda la cuenta; con una libreta, sólo esa
/// (mismo criterio «cuenta/libreta» del alcance del CSV). La escritura es
/// atómica y sigue el enlace simbólico, igual que `exportar_csv`.
#[tauri::command]
pub async fn exportar_vcf(
    app: tauri::AppHandle,
    ruta: String,
    account_id: String,
    address_book_id: String,
) -> Result<(), String> {
    use tauri_plugin_vsk_store::StoreExt;
    let tarjetas =
        crate::store::read_raw_cards(&app.store(), &account_id, &address_book_id).await?;
    let vcards: Vec<String> = tarjetas.into_iter().map(|t| t.vcard).collect();
    // Armar y escribir en un hilo aparte, como el CSV: con muchas tarjetas el
    // ensamblado no es trivial y la escritura puede ir a un disco lento o de red.
    tauri::async_runtime::spawn_blocking(move || exportar_vcf_impl(&ruta, &ensamblar_vcf(&vcards)))
        .await
        .map_err(|e| format!("no se pudo guardar el archivo: {e}"))?
}

/// El trabajo en disco de `exportar_vcf`, aparte para probarlo sin runtime async.
/// Sin BOM: vCard es UTF-8 sin marca de orden (RFC 6350); el BOM del CSV era
/// sólo para que Excel abriera los acentos.
fn exportar_vcf_impl(ruta: &str, vcf: &str) -> Result<(), String> {
    escribir_atomico(std::path::Path::new(ruta), vcf.as_bytes())
        .map_err(|e| format!("no se pudo guardar el archivo: {e}"))
}

/// Junta los vCard crudos de varias tarjetas en el contenido de un `.vcf`.
///
/// Función pura: no toca el almacén ni el disco, para poder probarla sola.
///
/// No **reescribe** el contenido de cada tarjeta —se preserva tal cual se
/// guardó: plegado, orden de propiedades y escapes—; sólo normaliza lo mínimo
/// que pide el formato:
///
/// - finales de línea **CRLF** (RFC 6350), reemplazando los `\n`/`\r` sueltos;
/// - sin líneas en blanco al principio ni al final del bloque, para que dos
///   tarjetas no se peguen ni quede una separación de más;
/// - un salto final después de la última.
///
/// Una entrada vacía, en blanco, o sin un `BEGIN:VCARD`/`END:VCARD` **se
/// saltea** en vez de abortar la exportación: igual que el parseo (ver
/// `vcard`), una tarjeta rota no debe impedir exportar las otras trescientas.
/// Los espacios colgantes del interior **no** se recortan: en una línea plegada
/// el último carácter antes del pliegue puede ser un espacio del valor.
pub fn ensamblar_vcf(vcards: &[String]) -> String {
    let mut salida = String::new();
    for cruda in vcards {
        if let Some(bloque) = normalizar_vcard(cruda) {
            salida.push_str(&bloque);
            salida.push_str("\r\n");
        }
    }
    salida
}

/// Normaliza una tarjeta cruda a líneas unidas con CRLF, o `None` si está vacía
/// o no es una tarjeta (sin `BEGIN:VCARD` y `END:VCARD`).
fn normalizar_vcard(cruda: &str) -> Option<String> {
    // Partir por `\n` y sacar el `\r` colgante normaliza CRLF y LF por igual.
    let lineas: Vec<&str> = cruda
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    // Sin las líneas en blanco de los bordes: las del final pegarían la
    // separación al salto que agrega `ensamblar_vcf`, y las del principio
    // meterían una línea vacía entre dos tarjetas.
    let inicio = lineas.iter().position(|l| !l.trim().is_empty())?;
    let fin = lineas.iter().rposition(|l| !l.trim().is_empty())?;
    let cuerpo = &lineas[inicio..=fin];
    // Tiene que ser una tarjeta: delimitadores presentes (sin distinguir
    // mayúsculas, que vCard no distingue en los nombres de propiedad).
    let tiene = |clave: &str| cuerpo.iter().any(|l| l.trim().eq_ignore_ascii_case(clave));
    if !tiene("BEGIN:VCARD") || !tiene("END:VCARD") {
        return None;
    }
    Some(cuerpo.join("\r\n"))
}

/// Escribe a un temporal en el mismo directorio y renombra, siguiendo el enlace
/// simbólico. `rename` sólo es atómico dentro del mismo sistema de archivos, por
/// eso el temporal va al lado del destino.
fn escribir_atomico(ruta: &std::path::Path, contenido: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    // `rename` no sigue el destino: sin canonicalizar, guardar sobre un enlace
    // lo reemplazaría por un archivo común. Si la ruta no existe todavía —lo
    // normal al exportar— `canonicalize` falla y se usa tal cual.
    let real = std::fs::canonicalize(ruta).unwrap_or_else(|_| ruta.to_path_buf());
    let directorio = real.parent().unwrap_or_else(|| std::path::Path::new("."));
    let nombre = real
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "contactos.csv".to_string());
    let temporal = directorio.join(format!(".{nombre}.vasak-contacts.{}", std::process::id()));

    // Los permisos del archivo que se reemplaza, para restaurarlos: reexportar
    // sobre un CSV que estaba en 0600 no debe dejarlo legible por todos.
    let permisos = std::fs::metadata(&real).ok().map(|m| m.permissions());

    let resultado = (|| -> std::io::Result<()> {
        let mut archivo = crear_temporal(&temporal)?;
        archivo.write_all(contenido)?;
        // Los permisos se cambian por el descriptor abierto, no por la ruta:
        // `std::fs::set_permissions` sobre la ruta seguiría un enlace que un
        // atacante hubiera puesto en el nombre del temporal tras crearlo, y le
        // aplicaría a un archivo ajeno los permisos (quizá de lectura pública) del
        // CSV anterior. El `File` apunta al inodo que creamos, no a lo que diga el
        // nombre ahora.
        if let Some(permisos) = &permisos {
            archivo.set_permissions(permisos.clone())?;
        }
        // `sync_all` después de los permisos, para que también persistan.
        archivo.sync_all()?;
        std::fs::rename(&temporal, &real)?;
        // Sincronizar el directorio: en Linux, sincronizar el archivo no asegura
        // que su entrada —la que creó el `rename`— sobreviva a un corte de luz.
        if let Ok(dir) = std::fs::File::open(directorio) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporal);
    }
    resultado
}

/// Crea el temporal cerrando la carrera: `create_new` (`O_EXCL`) no sigue un
/// enlace pre-plantado con el nombre predecible del temporal ni pisa un archivo
/// que ya esté, y `mode(0o600)` lo hace privado desde que nace —datos personales
/// en un directorio compartido-escribible no deben quedar legibles por nadie—.
/// Un temporal huérfano del mismo pid se saca una vez y se reintenta; si
/// reaparece, se falla antes que seguir algo que no creamos nosotros.
fn crear_temporal(temporal: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut opciones = std::fs::OpenOptions::new();
    opciones.write(true).create_new(true).mode(0o600);
    match opciones.open(temporal) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(temporal)?;
            opciones.open(temporal)
        }
        otro => otro,
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

    /// Directorio temporal propio de cada prueba, para no pisar a las demás.
    fn directorio(nombre: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vasak-contacts-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Exportar escribe el CSV con un BOM delante —sin él Excel rompe los
    /// acentos— y no deja el temporal al lado.
    #[test]
    fn exportar_csv_escribe_con_bom_y_sin_dejar_temporales() {
        let dir = directorio("bom");
        let ruta = dir.join("contactos.csv");

        exportar_csv_impl(&ruta.to_string_lossy(), "Name\r\nÁlvaro").unwrap();

        assert_eq!(
            std::fs::read(&ruta).unwrap(),
            "\u{feff}Name\r\nÁlvaro".as_bytes()
        );
        let restos: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(restos.len(), 1, "sólo el CSV, sin temporales");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Exportar sobre un archivo que ya existe lo reemplaza entero.
    #[test]
    fn escribir_atomico_reemplaza_el_contenido_anterior() {
        let dir = directorio("reemplazo");
        let ruta = dir.join("a.csv");
        std::fs::write(&ruta, "un contenido viejo y bastante más largo").unwrap();

        escribir_atomico(&ruta, b"nuevo").unwrap();

        assert_eq!(std::fs::read(&ruta).unwrap(), b"nuevo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Guardar sobre un enlace simbólico escribe el archivo apuntado y deja el
    /// enlace como enlace; sin seguirlo, `rename` lo cambiaría por un archivo.
    #[cfg(unix)]
    #[test]
    fn escribir_atomico_sigue_el_enlace_simbolico() {
        let dir = directorio("enlace");
        let real = dir.join("real.csv");
        let enlace = dir.join("enlace.csv");
        std::fs::write(&real, "viejo").unwrap();
        std::os::unix::fs::symlink(&real, &enlace).unwrap();

        escribir_atomico(&enlace, b"nuevo").unwrap();

        assert!(std::fs::symlink_metadata(&enlace)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(&real).unwrap(), b"nuevo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un enlace pre-plantado con el nombre predecible del temporal no se sigue:
    /// el archivo al que apunta (la víctima) queda intacto.
    #[cfg(unix)]
    #[test]
    fn escribir_atomico_no_sigue_un_temporal_plantado_como_enlace() {
        let dir = directorio("temporal-plantado");
        let victima = dir.join("victima.txt");
        std::fs::write(&victima, "secreto").unwrap();
        let temporal = dir.join(format!(".export.csv.vasak-contacts.{}", std::process::id()));
        std::os::unix::fs::symlink(&victima, &temporal).unwrap();

        let destino = dir.join("export.csv");
        let _ = escribir_atomico(&destino, b"nuevo");

        assert_eq!(std::fs::read(&victima).unwrap(), b"secreto");
        assert_eq!(std::fs::read(&destino).unwrap(), b"nuevo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Reexportar sobre un CSV en 0600 lo deja en 0600, no en lo que diga el umask.
    #[cfg(unix)]
    #[test]
    fn escribir_atomico_conserva_los_permisos_del_archivo_reemplazado() {
        use std::os::unix::fs::PermissionsExt;
        let dir = directorio("permisos");
        let ruta = dir.join("p.csv");
        std::fs::write(&ruta, "viejo").unwrap();
        std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o640)).unwrap();

        escribir_atomico(&ruta, b"nuevo").unwrap();

        let modo = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o640);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tarjeta(nombre: &str) -> String {
        format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:{nombre}\r\nEND:VCARD")
    }

    /// Cada tarjeta sale con CRLF aunque se haya guardado con LF, y con un
    /// salto final tras la última.
    #[test]
    fn ensamblar_vcf_normaliza_a_crlf_y_cierra_con_salto() {
        let salida = ensamblar_vcf(&["BEGIN:VCARD\nFN:Ana\nEND:VCARD".to_string()]);

        assert_eq!(salida, "BEGIN:VCARD\r\nFN:Ana\r\nEND:VCARD\r\n");
        assert!(!salida.replace("\r\n", "").contains('\n'), "sin LF sueltos");
    }

    /// Dos tarjetas quedan en líneas separadas y sin líneas en blanco de más,
    /// aunque la primera traiga saltos de sobra al final. Concatenar sin
    /// normalizar pegaría `END:VCARD` con `BEGIN:VCARD` o metería una vacía.
    #[test]
    fn ensamblar_vcf_no_pega_ni_separa_de_mas_dos_tarjetas() {
        let a = format!("\n\n{}\r\n\r\n\n", tarjeta("Ana"));
        let salida = ensamblar_vcf(&[a, tarjeta("Beto")]);

        assert_eq!(
            salida,
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\nEND:VCARD\r\n\
             BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Beto\r\nEND:VCARD\r\n"
        );
    }

    /// Una entrada vacía, en blanco o sin delimitadores se saltea; las
    /// válidas siguen saliendo. Los delimitadores no distinguen mayúsculas.
    #[test]
    fn ensamblar_vcf_saltea_las_entradas_invalidas() {
        let entradas = [
            String::new(),
            "  \r\n \n".to_string(),
            "FN:Sin delimitadores".to_string(),
            "BEGIN:VCARD\r\nFN:Sin fin".to_string(),
            "FN:Sin inicio\r\nEND:VCARD".to_string(),
            tarjeta("Ana"),
            "begin:vcard\nfn:Beto\nend:vcard".to_string(),
        ];

        let salida = ensamblar_vcf(&entradas);

        assert_eq!(
            salida,
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\nEND:VCARD\r\n\
             begin:vcard\r\nfn:Beto\r\nend:vcard\r\n"
        );
        assert_eq!(ensamblar_vcf(&[]), "");
        assert_eq!(ensamblar_vcf(&[String::new(), "  ".into()]), "");
    }

    /// El interior se conserva byte a byte salvo el fin de línea: el pliegue
    /// y los espacios colgantes no se tocan (el último carácter antes de un
    /// pliegue puede ser un espacio del valor).
    #[test]
    fn ensamblar_vcf_preserva_el_interior_plegado_y_los_espacios() {
        let cruda = "BEGIN:VCARD\nPHOTO;ENCODING=b:AAAA\n BBBB\n CCCC\nNOTE:con espacio \n y sigue\nEND:VCARD";

        let salida = ensamblar_vcf(&[cruda.to_string()]);

        assert_eq!(salida, format!("{}\r\n", cruda.replace('\n', "\r\n")));
        assert!(salida.contains("NOTE:con espacio \r\n y sigue"));
    }

    /// Lo ensamblado vuelve a separarse en las mismas tarjetas y cada una se
    /// lee con su nombre.
    #[test]
    fn ensamblar_vcf_ida_y_vuelta_con_el_parser() {
        let entradas = vec![
            tarjeta("Ana"),
            "BEGIN:VCARD\nVERSION:3.0\nFN:Beto\nEND:VCARD\n\n".to_string(),
            tarjeta("Carla"),
        ];

        let vcf = ensamblar_vcf(&entradas);
        let tarjetas = crate::vcard::split_cards(&vcf);

        assert_eq!(tarjetas.len(), 3);
        let nombres: Vec<String> = tarjetas
            .iter()
            .map(|t| crate::vcard::contact_from(t, "u").unwrap().name)
            .collect();
        assert_eq!(nombres, ["Ana", "Beto", "Carla"]);
    }

    /// Exportar el `.vcf` escribe el contenido tal cual, SIN BOM (a diferencia
    /// del CSV: vCard es UTF-8 sin marca) y sin dejar temporales al lado.
    #[test]
    fn exportar_vcf_escribe_sin_bom_y_sin_dejar_temporales() {
        let dir = directorio("vcf");
        let ruta = dir.join("contactos.vcf");
        let vcf = "BEGIN:VCARD\r\nFN:Álvaro\r\nEND:VCARD\r\n";

        exportar_vcf_impl(&ruta.to_string_lossy(), vcf).unwrap();

        let bytes = std::fs::read(&ruta).unwrap();
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "sin BOM");
        assert_eq!(bytes, vcf.as_bytes());
        let restos: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(restos.len(), 1, "sólo el .vcf, sin temporales");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
