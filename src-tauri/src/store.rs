//! Los contactos, tal como ya están sincronizados en el sistema.
//!
//! `vasak-accounts-sync` mantiene una base cifrada con las libretas de cada
//! cuenta y la actualiza solo, en segundo plano. Esta ventana la lee por el bus
//! de sesión —`ar.net.vasak.os.AccountsStore`— y **no espera a la red**: la
//! agenda aparece al abrir.
//!
//! Lo que se pide son las **tarjetas crudas** (`ListContactCards`) y no el
//! resumen de `GetContact`: la ventana muestra la tarjeta entera —direcciones,
//! fechas, fotos, redes—, y la lee con el mismo intérprete de vCard que usaba
//! con lo que bajaba del servidor. Es texto que escribió cualquiera, igual que
//! antes.
//!
//! Lo nuevo se avisa con `Changed("contacts", …)`, que se pasa a la ventana
//! como [`CHANGED_EVENT`]; el botón «Actualizar» pide `RequestSync`.
//!
//! La primera lectura puede abrir el diálogo de `vasak-permissions`
//! (`store.contacts`): por eso las llamadas no tienen tope de tiempo.

use serde::{Deserialize, Serialize};

use crate::carddav::Libreta;
use crate::comandos::LecturaDeCuenta;
use crate::vcard;

const SERVICE: &str = "ar.net.vasak.os.AccountsSync";
const PATH: &str = "/ar/net/vasak/os/AccountsStore";
const INTERFACE: &str = "ar.net.vasak.os.AccountsStore";

/// El aviso a la ventana de que cambió lo guardado de los contactos.
pub const CHANGED_EVENT: &str = "contacts-changed";

/// El aviso a la ventana de que cambió el estado de la sincronización.
pub const STATUS_EVENT: &str = "contacts-sync-status";

/// Cuánto se juntan los avisos seguidos antes de pasarlos a la ventana.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(500);

/// Cuántas tarjetas se piden por página: el máximo que acepta el almacén.
const PAGE_SIZE: u32 = 1000;

/// Cuántas páginas se leen como mucho por cuenta. Cincuenta mil contactos no
/// es una agenda personal; se muestra lo que entró y se avisa.
const MAX_PAGES: usize = 50;

/// Lo más que manda el almacén por página: 4 MiB de filas, el cursor y el
/// envoltorio (`MAX_PAGE_REPLY_BYTES` de `vasak-accounts`).
const MAX_REPLY_BYTES: usize = 4 * 1024 * 1024 + 64 * 1024 + 64;

/// Lo más que guarda el almacén de una tarjeta. Una más grande no salió de él.
const MAX_CARD_BYTES: usize = 512 * 1024;

/// Una libreta como la lista el almacén.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StoredAddressBook {
    pub id: String,
    pub display_name: String,
}

/// Una tarjeta cruda del almacén.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StoredCard {
    pub id: String,
    #[serde(default)]
    pub address_book_id: String,
    #[serde(default)]
    pub href: String,
    pub vcard: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CardPage {
    items: Vec<StoredCard>,
    #[serde(default)]
    next_cursor: Option<String>,
}

async fn connect() -> Result<zbus::Connection, String> {
    zbus::Connection::session()
        .await
        .map_err(|e| format!("no se pudo contactar al almacén del sistema: {e}"))
}

async fn call<A>(metodo: &str, argumentos: &A) -> Result<String, String>
where
    A: serde::ser::Serialize + zbus::zvariant::DynamicType,
{
    connect()
        .await?
        .call_method(Some(SERVICE), PATH, Some(INTERFACE), metodo, argumentos)
        .await
        .map_err(|e| format!("{metodo}: {e}"))?
        .body()
        .deserialize::<String>()
        .map_err(|e| format!("respuesta inválida de {metodo}: {e}"))
        .and_then(capped)
}

/// Rechaza una respuesta más grande que la página más grande que manda el
/// almacén. El de verdad nunca pasa de ahí; algo que se hace pasar por él en el
/// bus de sesión podría mandar más de cien megas por página.
fn capped(json: String) -> Result<String, String> {
    if json.len() > MAX_REPLY_BYTES {
        return Err(format!(
            "el almacén mandó una respuesta de {} bytes, más de las que manda",
            json.len()
        ));
    }
    Ok(json)
}

/// La identidad de un contacto del almacén: la cuenta y dónde vive la
/// tarjeta en el servidor. Es lo que la ventana compara para seguir mostrando
/// el mismo contacto después de releer, así que no puede cambiar entre vueltas
/// —el número de la base sí podría— ni repetirse entre cuentas.
pub fn identity(account_id: &str, tarjeta: &StoredCard) -> String {
    let donde = if tarjeta.href.is_empty() {
        &tarjeta.id
    } else {
        &tarjeta.href
    };
    format!("store:{account_id}:{donde}")
}

/// Las libretas en la forma de la ventana.
pub fn address_books_from(guardadas: Vec<StoredAddressBook>) -> Vec<Libreta> {
    guardadas
        .into_iter()
        .map(|l| Libreta {
            url: l.id,
            nombre: l.display_name,
        })
        .collect()
}

/// Lee las tarjetas con el intérprete de la ventana. Las que no tienen nada
/// que mostrar se descartan, como las que venían del servidor.
///
/// Un recurso del servidor puede traer **varias tarjetas** —hay libretas
/// exportadas como un solo archivo—: se separan como con lo que bajaba por red,
/// y cada una es un contacto. Leído entero, las propiedades de una se mezclarían
/// con las de la otra: un nombre con el correo de otra persona.
pub fn contacts_from(account_id: &str, tarjetas: Vec<StoredCard>) -> Vec<vcard::Contact> {
    tarjetas
        .into_iter()
        .filter(|t| t.vcard.len() <= MAX_CARD_BYTES)
        .flat_map(|t| {
            let base = identity(account_id, &t);
            vcard::split_cards(&t.vcard)
                .into_iter()
                .enumerate()
                .filter_map(move |(i, card)| {
                    let id = if i == 0 {
                        base.clone()
                    } else {
                        format!("{base}#{i}")
                    };
                    vcard::contact_from(&card, &id)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Lo que hay de una cuenta en el almacén. `Ok(None)` si el almacén todavía
/// no tiene libretas de esa cuenta —el área no se encendió, la primera vuelta
/// no terminó—: quien llama va al servidor como antes.
pub async fn read_contacts(account_id: &str) -> Result<Option<LecturaDeCuenta>, String> {
    let json = call("ListAddressBooks", &(account_id,)).await?;
    let libretas: Vec<StoredAddressBook> = serde_json::from_str(&json)
        .map_err(|e| format!("no se pudieron leer las libretas: {e}"))?;
    let llamada = |cursor: String| async move {
        call(
            "ListContactCards",
            &(account_id, "", cursor.as_str(), PAGE_SIZE),
        )
        .await
    };
    read_store_pages(account_id, libretas, llamada).await
}

/// Lo de [`read_contacts`] sin el bus: las libretas ya leídas, y cómo pedir cada
/// página.
pub async fn read_store_pages<F, Fut>(
    account_id: &str,
    libretas: Vec<StoredAddressBook>,
    mut pagina: F,
) -> Result<Option<LecturaDeCuenta>, String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    if libretas.is_empty() {
        return Ok(None);
    }
    let mut lectura = LecturaDeCuenta {
        libretas: address_books_from(libretas),
        ..Default::default()
    };
    let mut cursor = String::new();
    for _ in 0..MAX_PAGES {
        let json = pagina(cursor.clone()).await?;
        let pagina: CardPage = serde_json::from_str(&json)
            .map_err(|e| format!("no se pudieron leer los contactos: {e}"))?;
        lectura
            .contactos
            .extend(contacts_from(account_id, pagina.items));
        match pagina.next_cursor {
            Some(siguiente) if !siguiente.is_empty() => cursor = siguiente,
            _ => return Ok(Some(lectura)),
        }
    }
    lectura
        .fallos
        .push("hay más contactos de los que se pueden mostrar".into());
    Ok(Some(lectura))
}

/// Una cuenta a la que no se le pudo pedir una vuelta, y por qué. La ventana
/// lo dice al lado de la cuenta: un «Actualizar» que no hizo nada y no lo dice
/// se lee como que no había nada nuevo.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncRequestFailure {
    pub account_id: String,
    /// `denied` (sin permiso), `too_soon` (se pidió hace un momento),
    /// `unavailable` (el sincronizador no está) o `failed`.
    pub reason: &'static str,
}

/// Por qué no se aceptó un pedido, a partir del nombre del error de D-Bus.
pub fn failure_reason(error_name: Option<&str>) -> &'static str {
    match error_name {
        Some("org.freedesktop.DBus.Error.AccessDenied") => "denied",
        Some("org.freedesktop.DBus.Error.LimitsExceeded") => "too_soon",
        Some("org.freedesktop.DBus.Error.ServiceUnknown")
        | Some("org.freedesktop.DBus.Error.NameHasNoOwner")
        | Some("org.freedesktop.DBus.Error.UnknownObject")
        | Some("org.freedesktop.DBus.Error.UnknownMethod") => "unavailable",
        _ => "failed",
    }
}

/// El nombre de un error de D-Bus, si lo tiene.
fn error_name(error: &zbus::Error) -> Option<String> {
    match error {
        zbus::Error::MethodError(name, _, _) => Some(name.to_string()),
        zbus::Error::FDO(e) => Some(zbus::DBusError::name(e.as_ref()).to_string()),
        _ => None,
    }
}

/// Le pide al sincronizador una vuelta ya de unas cuentas: el botón
/// «Actualizar». No espera a que termine: lo nuevo se avisa con [`CHANGED_EVENT`]
/// y el estado con [`STATUS_EVENT`]. Devuelve las cuentas a las que no se les
/// pudo pedir; un fallo de una no frena a las demás.
pub async fn request_sync(account_ids: &[String]) -> Vec<SyncRequestFailure> {
    let conexion = match connect().await {
        Ok(conexion) => conexion,
        Err(e) => {
            eprintln!("[contactos] no se pudo pedir una vuelta: {e}");
            return account_ids
                .iter()
                .map(|id| SyncRequestFailure {
                    account_id: id.clone(),
                    reason: "unavailable",
                })
                .collect();
        }
    };
    let mut failures = Vec::new();
    for id in account_ids {
        if let Err(e) = conexion
            .call_method(
                Some(SERVICE),
                PATH,
                Some(INTERFACE),
                "RequestSync",
                &(id.as_str(),),
            )
            .await
        {
            let reason = failure_reason(error_name(&e).as_deref());
            eprintln!("[contactos] no se pudo pedir una vuelta de una cuenta ({reason})");
            failures.push(SyncRequestFailure {
                account_id: id.clone(),
                reason,
            });
        }
    }
    failures
}

/// El estado de la sincronización de los contactos de una cuenta, como lo da
/// `GetStatus`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AccountSyncStatus {
    pub account_id: String,
    /// `off`, `pending`, `syncing`, `synced`, `unavailable` o `failed`.
    pub state: String,
    /// Qué pasó, si falló o no está disponible. Lo escribe el almacén.
    pub detail: String,
    /// Cuándo terminó la última vuelta buena (RFC 3339), si hubo.
    pub last_synced_at: Option<String>,
}

/// El estado del área `area` de cada cuenta, de la respuesta de `GetStatus`.
/// Las cuentas sin esa área no aparecen.
pub fn statuses_from(json: &str, area: &str) -> Result<Vec<AccountSyncStatus>, String> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| format!("no se pudo leer el estado del almacén: {e}"))?;
    let accounts = value
        .get("accounts")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    let text =
        |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Ok(accounts
        .iter()
        .filter_map(|account| {
            let shown = account.get(area)?;
            Some(AccountSyncStatus {
                account_id: text(account, "account_id")?,
                state: text(shown, "state").unwrap_or_else(|| "off".into()),
                detail: text(shown, "detail").unwrap_or_default(),
                last_synced_at: text(shown, "last_synced_at"),
            })
        })
        .collect())
}

/// El estado de la sincronización de los contactos de cada cuenta.
pub async fn sync_status() -> Result<Vec<AccountSyncStatus>, String> {
    let json = call("GetStatus", &()).await?;
    statuses_from(&json, "contacts")
}

/// La regla de la señal que interesa: `Changed` del almacén, sólo de los
/// contactos. El bus filtra por el primer argumento, así que un lote de correo
/// o de calendario no despierta a esta ventana.
pub fn change_rule() -> zbus::Result<zbus::MatchRule<'static>> {
    Ok(zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        // Sólo del dueño del nombre del sincronizador y de su objeto: cualquier
        // proceso de la sesión puede emitir una señal con esta interfaz, y cada
        // una hace releer la agenda entera.
        .sender(SERVICE)?
        .path(PATH)?
        .interface(INTERFACE)?
        .member("Changed")?
        .arg(0, "contacts")?
        .build())
}

/// Pasa a la ventana cada `Changed("contacts", …)` del almacén.
pub fn watch(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // En bucle: el bus se puede cortar, y con un solo intento la ventana
        // dejaría de enterarse de lo nuevo hasta que se la cierre.
        loop {
            if let Err(e) = follow(&app).await {
                eprintln!("[contactos] no se pueden recibir avisos del almacén: {e}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

/// La señal de que cambió el estado de alguna base: `StatusChanged`, sin
/// argumentos, también sólo del sincronizador y de su objeto.
fn status_rule() -> Result<zbus::MatchRule<'static>, String> {
    Ok(zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(SERVICE)
        .and_then(|r| r.path(PATH))
        .and_then(|r| r.interface(INTERFACE))
        .and_then(|r| r.member("StatusChanged"))
        .map_err(|e| format!("no se pudo armar el filtro del estado: {e}"))?
        .build())
}

/// Qué avisó el almacén.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    Changed,
    Status,
}

/// Qué eventos recibe la ventana por una ráfaga de avisos: uno de cada clase
/// que llegó, nunca uno por aviso. Un `StatusChanged` solo no relee la agenda,
/// y un `Changed` solo no relee el estado.
fn events_for(burst: &[Notice]) -> Vec<&'static str> {
    let mut events = Vec::new();
    if burst.contains(&Notice::Changed) {
        events.push(CHANGED_EVENT);
    }
    if burst.contains(&Notice::Status) {
        events.push(STATUS_EVENT);
    }
    events
}

async fn follow(app: &tauri::AppHandle) -> Result<(), String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let conexion = connect().await?;
    let changes = zbus::MessageStream::for_match_rule(
        change_rule().map_err(|e| format!("no se pudo armar el filtro: {e}"))?,
        &conexion,
        None,
    )
    .await
    .map_err(|e| format!("no se pudo escuchar al almacén: {e}"))?
    .map(|m| m.map(|_| Notice::Changed));
    let statuses = zbus::MessageStream::for_match_rule(status_rule()?, &conexion, None)
        .await
        .map_err(|e| format!("no se pudo escuchar el estado del almacén: {e}"))?
        .map(|m| m.map(|_| Notice::Status));
    let mut avisos = futures_util::stream::select(changes, statuses);
    while let Some(aviso) = avisos.next().await {
        let Ok(first) = aviso else {
            continue;
        };
        // Una vuelta escribe de a lotes y avisa por cada uno: los seguidos se
        // juntan, y la ventana recibe un aviso de cada clase.
        tokio::time::sleep(SETTLE).await;
        let mut seen = vec![first];
        while let Some(Some(next)) = futures_util::FutureExt::now_or_never(avisos.next()) {
            if let Ok(kind) = next {
                seen.push(kind);
            }
        }
        for event in events_for(&seen) {
            if let Err(e) = app.emit(event, ()) {
                eprintln!("[contactos] no se pudo avisar a la ventana: {e}");
            }
        }
    }
    Err("el bus de sesión cerró la conexión".into())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn card(id: &str, href: &str, vcard: &str) -> StoredCard {
        StoredCard {
            id: id.into(),
            address_book_id: "1".into(),
            href: href.into(),
            vcard: vcard.into(),
        }
    }

    fn named(n: usize) -> String {
        format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Contacto {n}\r\nEND:VCARD\r\n")
    }

    /// Una página de `ListContactCards` como la escribe `vasak-accounts`.
    fn page_json(names: &[usize], next: Option<&str>) -> String {
        let items: Vec<serde_json::Value> = names
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.to_string(),
                    "address_book_id": "1",
                    "href": format!("https://x/a/{n}.vcf"),
                    "vcard": named(*n),
                })
            })
            .collect();
        serde_json::json!({ "items": items, "next_cursor": next }).to_string()
    }

    fn one_book() -> Vec<StoredAddressBook> {
        vec![StoredAddressBook {
            id: "1".into(),
            display_name: "Personal".into(),
        }]
    }

    /// Corre `leer` con páginas armadas de antemano y devuelve lo que leyó y
    /// los cursores con que se pidió cada página.
    async fn read_pages(
        books: Vec<StoredAddressBook>,
        pages: Vec<Result<String, String>>,
    ) -> (Result<Option<LecturaDeCuenta>, String>, Vec<String>) {
        let asked = RefCell::new(Vec::new());
        let pages = RefCell::new(pages.into_iter());
        let result = read_store_pages("cuenta", books, |cursor| {
            asked.borrow_mut().push(cursor);
            let next = pages
                .borrow_mut()
                .next()
                .unwrap_or_else(|| Err("se pidió una página de más".into()));
            async move { next }
        })
        .await;
        (result, asked.into_inner())
    }

    fn names(lectura: &LecturaDeCuenta) -> Vec<&str> {
        lectura.contactos.iter().map(|c| c.name.as_str()).collect()
    }

    // ── La identidad ───────────────────────────────────────────────────────

    #[test]
    fn la_identidad_es_la_cuenta_y_donde_vive_la_tarjeta() {
        let t = card("7", "https://x/a/ana.vcf", &named(1));
        assert_eq!(identity("cuenta", &t), "store:cuenta:https://x/a/ana.vcf");
    }

    /// El número de la base puede cambiar entre vueltas; el `href` no. Con el
    /// mismo `href`, la ventana sigue mostrando al mismo contacto.
    #[test]
    fn la_identidad_no_cambia_si_cambia_el_numero_de_la_base() {
        let before = card("7", "https://x/a/ana.vcf", &named(1));
        let after = card("912", "https://x/a/ana.vcf", &named(1));
        assert_eq!(identity("cuenta", &before), identity("cuenta", &after));
    }

    #[test]
    fn sin_href_la_identidad_usa_el_numero_de_la_base() {
        let t = card("7", "", &named(1));
        assert_eq!(identity("cuenta", &t), "store:cuenta:7");
        assert_ne!(
            identity("cuenta", &t),
            identity("cuenta", &card("8", "", &named(1)))
        );
    }

    /// Dos cuentas pueden tener la misma tarjeta en la misma dirección (dos
    /// personas en el mismo servidor, la misma libreta compartida): no son el
    /// mismo contacto para la ventana.
    #[test]
    fn la_misma_tarjeta_en_dos_cuentas_no_tiene_la_misma_identidad() {
        let t = card("7", "https://x/a/ana.vcf", &named(1));
        assert_ne!(identity("una", &t), identity("otra", &t));
        let bare = card("7", "", &named(1));
        assert_ne!(identity("una", &bare), identity("otra", &bare));
    }

    // ── Las libretas y las tarjetas ────────────────────────────────────────

    #[test]
    fn las_libretas_guardadas_pasan_a_la_forma_de_la_ventana() {
        let books = address_books_from(vec![
            StoredAddressBook {
                id: "3".into(),
                display_name: "Trabajo".into(),
            },
            StoredAddressBook {
                id: "1".into(),
                display_name: "Personal".into(),
            },
        ]);
        let shown: Vec<(&str, &str)> = books
            .iter()
            .map(|b| (b.url.as_str(), b.nombre.as_str()))
            .collect();
        assert_eq!(shown, vec![("3", "Trabajo"), ("1", "Personal")]);
    }

    /// La tarjeta se lee con el intérprete de la ventana, así que no se pierde
    /// ningún campo de los que mostraba con lo que bajaba del servidor:
    /// direcciones, cumpleaños, foto, redes.
    #[test]
    fn una_tarjeta_guardada_se_lee_entera_con_el_interprete_de_la_ventana() {
        let raw = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:u-ana\r\nFN:Ana Pérez\r\n\
                   N:Pérez;Ana;;;\r\nNICKNAME:Anita\r\nORG:Vasak\r\nTITLE:Jefa de soporte\r\n\
                   EMAIL;TYPE=work:ana@x.com\r\nTEL;TYPE=cell:+54 11 5555 0000\r\n\
                   ADR;TYPE=HOME:;;Av. Siempreviva 742;Springfield;Buenos Aires;B1636;Argentina\r\n\
                   BDAY:1990-04-15\r\nURL:https://ana.example.com\r\n\
                   PHOTO;ENCODING=b;TYPE=PNG:iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==\r\n\
                   X-SOCIALPROFILE;TYPE=mastodon:@ana@mastodon.social\r\n\
                   NOTE:Una nota\r\nEND:VCARD\r\n";
        let stored = card("7", "https://x/a/ana.vcf", raw);
        let contacts = contacts_from("cuenta", vec![stored.clone()]);
        assert_eq!(contacts.len(), 1);
        let ana = &contacts[0];

        // Lo mismo que daría el intérprete de la ventana, campo por campo.
        let expected =
            vcard::contact_from(raw, &identity("cuenta", &stored)).expect("la tarjeta se lee");
        assert_eq!(ana, &expected);

        assert_eq!(ana.name, "Ana Pérez");
        assert_eq!(ana.url, "store:cuenta:https://x/a/ana.vcf");
        assert_eq!(ana.nickname, "Anita");
        assert_eq!(ana.title, "Jefa de soporte");
        assert_eq!(ana.emails[0].value, "ana@x.com");
        assert_eq!(ana.addresses.len(), 1, "las direcciones");
        assert!(ana.birthday.is_some(), "el cumpleaños");
        assert!(ana.photo.starts_with("data:image/"), "la foto");
        assert_eq!(ana.social.len(), 1, "las redes");
        assert_eq!(ana.websites.len(), 1);
    }

    /// Las que no tienen nada que mostrar se descartan, como las del
    /// servidor; las demás siguen, en el orden en que vinieron.
    #[test]
    fn las_tarjetas_sin_nada_que_mostrar_se_descartan() {
        let contacts = contacts_from(
            "cuenta",
            vec![
                card("1", "https://x/a/1.vcf", &named(1)),
                card(
                    "2",
                    "https://x/a/2.vcf",
                    "BEGIN:VCARD\r\nVERSION:3.0\r\nEND:VCARD\r\n",
                ),
                card("3", "https://x/a/3.vcf", "esto no es una tarjeta"),
                card("4", "https://x/a/4.vcf", &named(4)),
            ],
        );
        let shown: Vec<(&str, &str)> = contacts
            .iter()
            .map(|c| (c.name.as_str(), c.url.as_str()))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("Contacto 1", "store:cuenta:https://x/a/1.vcf"),
                ("Contacto 4", "store:cuenta:https://x/a/4.vcf"),
            ]
        );
    }

    // ── Las formas del almacén ─────────────────────────────────────────────

    /// `ListAddressBooks` contesta `[{id, display_name, contacts}]`: se lee,
    /// sin trabarse con el campo que no se usa.
    #[test]
    fn se_lee_la_respuesta_de_list_address_books() {
        let json = r#"[{"id":"3","display_name":"Trabajo","contacts":12},
                       {"id":"1","display_name":"Personal","contacts":0}]"#;
        let books: Vec<StoredAddressBook> = serde_json::from_str(json).unwrap();
        assert_eq!(
            books,
            vec![
                StoredAddressBook {
                    id: "3".into(),
                    display_name: "Trabajo".into(),
                },
                StoredAddressBook {
                    id: "1".into(),
                    display_name: "Personal".into(),
                },
            ]
        );
        let none: Vec<StoredAddressBook> = serde_json::from_str("[]").unwrap();
        assert!(none.is_empty());
    }

    /// `ListContactCards` contesta `{items: [{id, address_book_id, href,
    /// vcard}], next_cursor}`, con `next_cursor` en `null` al final.
    #[tokio::test]
    async fn se_lee_la_respuesta_de_list_contact_cards() {
        let raw = "BEGIN:VCARD\r\nFN:Ana \\\"la\\\" Pérez\r\nEND:VCARD\r\n";
        let json = serde_json::json!({
            "items": [{
                "id": "7",
                "address_book_id": "1",
                "href": "https://x/a/ana.vcf",
                "vcard": raw,
            }],
            "next_cursor": null,
        })
        .to_string();
        let (result, asked) = read_pages(one_book(), vec![Ok(json)]).await;
        let lectura = result.unwrap().expect("había libretas");
        assert_eq!(asked, vec![String::new()]);
        assert_eq!(lectura.contactos.len(), 1);
        assert_eq!(lectura.contactos[0].url, "store:cuenta:https://x/a/ana.vcf");
        assert_eq!(
            lectura.contactos[0],
            vcard::contact_from(raw, "store:cuenta:https://x/a/ana.vcf").unwrap()
        );
        assert_eq!(lectura.libretas[0].nombre, "Personal");
        assert!(lectura.fallos.is_empty());
    }

    // ── Leer una cuenta ────────────────────────────────────────────────────

    /// Sin libretas en el almacén —el área no se encendió, la primera vuelta
    /// no terminó— no se lee nada y se contesta `None`, para que quien llama
    /// vaya al servidor. Una agenda vacía no es «todavía no sé».
    #[tokio::test]
    async fn sin_libretas_en_el_almacen_no_se_lee_y_se_va_al_servidor() {
        let (result, asked) = read_pages(Vec::new(), vec![Ok(page_json(&[1], None))]).await;
        assert_eq!(result, Ok(None));
        assert!(asked.is_empty(), "no se pidió ninguna página");
    }

    /// Una libreta sin contactos sí es una respuesta: la agenda está vacía.
    #[tokio::test]
    async fn una_libreta_vacia_es_una_agenda_vacia_y_no_ir_al_servidor() {
        let (result, _) = read_pages(one_book(), vec![Ok(page_json(&[], None))]).await;
        let lectura = result.unwrap().expect("había libretas");
        assert!(lectura.contactos.is_empty());
        assert_eq!(lectura.libretas.len(), 1);
    }

    /// Las páginas se siguen con el cursor que dio la anterior, hasta que no
    /// hay más.
    #[tokio::test]
    async fn las_paginas_se_siguen_con_su_cursor_hasta_el_final() {
        let (result, asked) = read_pages(
            one_book(),
            vec![
                Ok(page_json(&[1, 2], Some("c1"))),
                Ok(page_json(&[3], Some("c2"))),
                Ok(page_json(&[4, 5], None)),
            ],
        )
        .await;
        let lectura = result.unwrap().unwrap();
        assert_eq!(asked, vec!["", "c1", "c2"]);
        assert_eq!(
            names(&lectura),
            vec![
                "Contacto 1",
                "Contacto 2",
                "Contacto 3",
                "Contacto 4",
                "Contacto 5"
            ]
        );
        assert!(lectura.fallos.is_empty());
    }

    /// Un cursor vacío también es el final: pedir de nuevo con él sería pedir
    /// la primera página otra vez, para siempre.
    #[tokio::test]
    async fn un_cursor_vacio_termina_la_lectura() {
        let (result, asked) = read_pages(
            one_book(),
            vec![Ok(page_json(&[1], Some(""))), Ok(page_json(&[1], None))],
        )
        .await;
        assert_eq!(asked.len(), 1);
        assert_eq!(names(&result.unwrap().unwrap()), vec!["Contacto 1"]);

        // Y sin `next_cursor` en la respuesta, igual.
        let json = serde_json::json!({ "items": [] }).to_string();
        let (result, asked) = read_pages(one_book(), vec![Ok(json)]).await;
        assert_eq!(asked.len(), 1);
        assert!(result.unwrap().is_some());
    }

    /// Un almacén que nunca termina de dar páginas no cuelga la ventana: se
    /// leen hasta `MAX_PAGINAS`, se muestra lo que entró y se avisa.
    #[tokio::test]
    async fn al_llegar_al_tope_de_paginas_se_muestra_lo_leido_y_se_avisa() {
        let pages = (0..MAX_PAGES + 5)
            .map(|n| Ok(page_json(&[n], Some(&format!("c{n}")))))
            .collect();
        let (result, asked) = read_pages(one_book(), pages).await;
        let lectura = result.unwrap().unwrap();
        assert_eq!(asked.len(), MAX_PAGES);
        assert_eq!(lectura.contactos.len(), MAX_PAGES);
        assert_eq!(lectura.fallos.len(), 1, "un aviso de que hay más");
    }

    /// Una página que no llega, o que llega rota, es un error de la lectura
    /// entera —para que quien llama vaya al servidor—, nunca una agenda a
    /// medias que parezca completa.
    #[tokio::test]
    async fn una_pagina_que_falla_o_llega_rota_es_un_error() {
        let (result, _) = read_pages(one_book(), vec![Err("el bus se cortó".into())]).await;
        assert!(result.unwrap_err().contains("el bus se cortó"));

        for garbage in [
            "no es json",
            "[]",
            r#"{"next_cursor":null}"#,
            r#"{"items":[{"id":"1"}]}"#,
        ] {
            let (result, _) = read_pages(one_book(), vec![Ok(garbage.into())]).await;
            assert!(result.is_err(), "{garbage:?} se tomó como una página");
        }

        let (result, asked) = read_pages(
            one_book(),
            vec![
                Ok(page_json(&[1, 2], Some("c1"))),
                Err("el bus se cortó".into()),
            ],
        )
        .await;
        assert_eq!(asked.len(), 2);
        assert!(
            result.is_err(),
            "la segunda página falló y se devolvió la primera"
        );
    }

    // ── El aviso de cambios ────────────────────────────────────────────────

    fn changed(area: &str) -> zbus::Message {
        zbus::Message::signal(PATH, INTERFACE, "Changed")
            .unwrap()
            .build(&(area, "cuenta", 3u64))
            .unwrap()
    }

    /// Sólo `Changed("contacts", …)` del almacén despierta a la ventana: un
    /// lote de correo o de calendario, otra señal o la misma señal de otra
    /// interfaz, no.
    #[test]
    fn la_regla_solo_deja_pasar_los_cambios_de_los_contactos() {
        let rule = change_rule().unwrap();
        assert!(rule.matches(&changed("contacts")).unwrap());
        assert!(!rule.matches(&changed("calendar")).unwrap());
        assert!(!rule.matches(&changed("email")).unwrap());

        let status = zbus::Message::signal(PATH, INTERFACE, "StatusChanged")
            .unwrap()
            .build(&())
            .unwrap();
        assert!(!rule.matches(&status).unwrap());

        let elsewhere = zbus::Message::signal(PATH, "ar.net.vasak.os.Otra", "Changed")
            .unwrap()
            .build(&("contacts", "cuenta", 3u64))
            .unwrap();
        assert!(!rule.matches(&elsewhere).unwrap());

        let text = rule.to_string();
        assert!(text.contains("type='signal'"), "{text}");
        assert!(text.contains("arg0='contacts'"), "{text}");
    }

    /// Un recurso con dos tarjetas son dos contactos, cada uno con lo suyo:
    /// leído entero, el nombre de una quedaba con el teléfono de la otra.
    #[test]
    fn un_recurso_con_dos_tarjetas_son_dos_contactos() {
        let two = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\nTEL:111\r\nEND:VCARD\r\n\
                   BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bruno\r\nTEL:222\r\nEND:VCARD\r\n";
        let contacts = contacts_from("a1", vec![card("7", "https://x/todos.vcf", two)]);
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].name, "Ana");
        assert_eq!(contacts[0].phones.len(), 1);
        assert_eq!(contacts[0].phones[0].value, "111");
        assert_eq!(contacts[1].name, "Bruno");
        assert_eq!(contacts[1].phones[0].value, "222");
        assert_ne!(contacts[0].url, contacts[1].url);
        assert_eq!(contacts[0].url, "store:a1:https://x/todos.vcf");
    }

    /// Una tarjeta más grande de lo que guarda el almacén no salió de él.
    #[test]
    fn una_tarjeta_mas_grande_que_las_del_almacen_se_descarta() {
        let huge = format!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Grande\r\nNOTE:{}\r\nEND:VCARD\r\n",
            "x".repeat(MAX_CARD_BYTES)
        );
        assert!(contacts_from("a1", vec![card("1", "h", &huge)]).is_empty());
        assert_eq!(
            contacts_from("a1", vec![card("1", "h", &named(1))]).len(),
            1
        );
    }

    /// Una respuesta más grande que la página más grande del almacén se
    /// rechaza antes de leerla.
    #[test]
    fn una_respuesta_mas_grande_que_las_del_almacen_se_rechaza() {
        assert!(capped("x".repeat(MAX_REPLY_BYTES)).is_ok());
        assert!(capped("x".repeat(MAX_REPLY_BYTES + 1)).is_err());
    }

    /// Cualquier proceso de la sesión puede emitir una señal con la interfaz
    /// del almacén: sólo cuenta la del dueño del nombre del sincronizador, y
    /// desde su objeto.
    #[test]
    fn solo_cuenta_el_changed_del_sincronizador_y_de_su_objeto() {
        let text = change_rule().unwrap().to_string();
        assert!(
            text.contains("sender='ar.net.vasak.os.AccountsSync'"),
            "{text}"
        );
        assert!(
            text.contains("path='/ar/net/vasak/os/AccountsStore'"),
            "{text}"
        );
    }

    /// El área de esta ventana en `GetStatus`, y otra que no tiene que mirar.
    const AREA: &str = "contacts";
    const OTHER_AREA: &str = "email";
    /// Los eventos que escucha la ventana (`src/composables`, `src/views`).
    const EXPECTED_CHANGED_EVENT: &str = "contacts-changed";
    const EXPECTED_STATUS_EVENT: &str = "contacts-sync-status";

    // ── El estado de la sincronización ─────────────────────────────────────

    /// La respuesta de `GetStatus` como la arma `visible_status` en
    /// `vasak-accounts/sync/src/store_api.rs`: con permiso sobre el área van
    /// `detail` y `last_synced_at`; sin permiso, sólo `state`.
    fn status_json() -> String {
        let mut con_permiso = serde_json::json!({
            "account_id": "google",
            "state": "ready",
            "detail": "",
            "size_bytes": 10,
        });
        con_permiso[AREA] = serde_json::json!({
            "state": "synced",
            "detail": "",
            "last_synced_at": "2026-10-13T17:05:00Z",
        });
        con_permiso[OTHER_AREA] = serde_json::json!({ "state": "failed" });
        let mut fallida = serde_json::json!({ "account_id": "nube", "state": "ready" });
        fallida[AREA] = serde_json::json!({
            "state": "failed",
            "detail": "el servidor rechazó la clave",
            "last_synced_at": null,
        });
        let mut de_otra_area = serde_json::json!({ "account_id": "otra", "state": "ready" });
        de_otra_area[OTHER_AREA] = serde_json::json!({ "state": "syncing" });
        let mut sin_permiso = serde_json::json!({ "account_id": "ajena", "state": "ready" });
        sin_permiso[AREA] = serde_json::json!({ "state": "pending" });
        serde_json::json!({
            "keyring": "unlocked",
            "accounts": [con_permiso, fallida, de_otra_area, sin_permiso],
        })
        .to_string()
    }

    fn estado(id: &str, state: &str, detail: &str, last: Option<&str>) -> AccountSyncStatus {
        AccountSyncStatus {
            account_id: id.into(),
            state: state.into(),
            detail: detail.into(),
            last_synced_at: last.map(str::to_string),
        }
    }

    #[test]
    fn el_estado_de_cada_cuenta_sale_de_su_area() {
        assert_eq!(
            statuses_from(&status_json(), AREA).unwrap(),
            vec![
                estado("google", "synced", "", Some("2026-10-13T17:05:00Z")),
                estado("nube", "failed", "el servidor rechazó la clave", None),
                // Sin permiso sobre el área el almacén manda sólo el estado: se
                // muestra igual, sin detalle ni hora.
                estado("ajena", "pending", "", None),
            ]
        );
    }

    /// Una cuenta que no tiene esta área —sólo otra— no tiene nada que decir
    /// acá: si apareciera, se le mostraría el estado de otra área.
    #[test]
    fn una_cuenta_sin_esta_area_no_aparece() {
        let ids: Vec<_> = statuses_from(&status_json(), AREA)
            .unwrap()
            .into_iter()
            .map(|s| s.account_id)
            .collect();
        assert!(!ids.contains(&"otra".to_string()), "{ids:?}");
        // Y de la que tiene las dos, el estado es el de esta área.
        let google = statuses_from(&status_json(), AREA).unwrap().remove(0);
        assert_eq!(google.state, "synced");
    }

    #[test]
    fn un_area_sin_estado_se_lee_apagada() {
        let mut cuenta = serde_json::json!({ "account_id": "google" });
        cuenta[AREA] = serde_json::json!({});
        let json = serde_json::json!({ "accounts": [cuenta] }).to_string();
        assert_eq!(
            statuses_from(&json, AREA).unwrap(),
            vec![estado("google", "off", "", None)]
        );
    }

    #[test]
    fn una_cuenta_sin_id_no_se_muestra() {
        let mut cuenta = serde_json::json!({ "state": "ready" });
        cuenta[AREA] = serde_json::json!({ "state": "synced" });
        let json = serde_json::json!({ "accounts": [cuenta] }).to_string();
        assert_eq!(statuses_from(&json, AREA).unwrap(), vec![]);
    }

    #[test]
    fn una_respuesta_que_no_es_json_es_un_error() {
        assert!(statuses_from("no es json", AREA).is_err());
    }

    /// Un almacén que todavía no tiene cuentas, o que contesta otra cosa en
    /// `accounts`, es ningún estado: no es un error que haya que mostrar.
    #[test]
    fn una_respuesta_sin_cuentas_no_tiene_estados() {
        assert_eq!(
            statuses_from(r#"{"keyring":"locked"}"#, AREA).unwrap(),
            vec![]
        );
        assert_eq!(statuses_from(r#"{"accounts":3}"#, AREA).unwrap(), vec![]);
        assert_eq!(statuses_from("[]", AREA).unwrap(), vec![]);
    }

    /// Lo que viaja a la ventana tiene que tener **exactamente** los nombres
    /// que lee `src/tools/sync-status.ts`: Tauri no los convierte.
    #[test]
    fn el_estado_y_los_rechazos_llegan_con_los_nombres_de_la_ventana() {
        let json =
            serde_json::to_value(estado("google", "synced", "", Some("2026-10-13T17:05:00Z")))
                .unwrap();
        let mut claves: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        claves.sort();
        assert_eq!(claves, ["account_id", "detail", "last_synced_at", "state"]);
        // Sin hora, `null`: el tipo de la ventana es `string | null`.
        let sin_hora = serde_json::to_value(estado("google", "pending", "", None)).unwrap();
        assert!(sin_hora["last_synced_at"].is_null());

        let rechazo = serde_json::to_value(SyncRequestFailure {
            account_id: "google".into(),
            reason: "denied",
        })
        .unwrap();
        assert_eq!(
            rechazo,
            serde_json::json!({ "account_id": "google", "reason": "denied" })
        );
    }

    // ── Lo que «Actualizar» no pudo pedir ──────────────────────────────────

    #[test]
    fn cada_rechazo_del_almacen_tiene_su_motivo() {
        for (nombre, motivo) in [
            ("org.freedesktop.DBus.Error.AccessDenied", "denied"),
            ("org.freedesktop.DBus.Error.LimitsExceeded", "too_soon"),
            ("org.freedesktop.DBus.Error.ServiceUnknown", "unavailable"),
            ("org.freedesktop.DBus.Error.NameHasNoOwner", "unavailable"),
            ("org.freedesktop.DBus.Error.UnknownObject", "unavailable"),
            ("org.freedesktop.DBus.Error.UnknownMethod", "unavailable"),
            ("org.freedesktop.DBus.Error.Failed", "failed"),
            ("ar.net.vasak.os.Error.Otro", "failed"),
        ] {
            assert_eq!(failure_reason(Some(nombre)), motivo, "{nombre}");
        }
        // Un error que no es de D-Bus —se cortó la conexión— también falló.
        assert_eq!(failure_reason(None), "failed");
    }

    /// Una respuesta de error de verdad, como la que llega por el bus. Así es
    /// como `call_method` devuelve el `LimitsExceeded` del almacén: un
    /// `MethodError`, no un `FDO`.
    fn error_reply(nombre: &str) -> zbus::Error {
        let pedido = zbus::Message::method_call(PATH, "RequestSync")
            .unwrap()
            .build(&("google",))
            .unwrap();
        let respuesta = zbus::Message::error(&pedido.header(), nombre)
            .unwrap()
            .build(&("una por cuenta cada minuto",))
            .unwrap();
        zbus::Error::from(respuesta)
    }

    #[test]
    fn el_motivo_sale_del_nombre_de_la_respuesta_de_error() {
        let limite = error_reply("org.freedesktop.DBus.Error.LimitsExceeded");
        assert!(matches!(limite, zbus::Error::MethodError(..)), "{limite:?}");
        assert_eq!(failure_reason(error_name(&limite).as_deref()), "too_soon");

        let sin_permiso = error_reply("org.freedesktop.DBus.Error.AccessDenied");
        assert_eq!(
            failure_reason(error_name(&sin_permiso).as_deref()),
            "denied"
        );

        // El mismo error ya convertido a `fdo::Error` da el mismo motivo.
        let fdo = zbus::Error::from(zbus::fdo::Error::AccessDenied("no".into()));
        assert!(matches!(fdo, zbus::Error::FDO(_)), "{fdo:?}");
        assert_eq!(failure_reason(error_name(&fdo).as_deref()), "denied");

        assert_eq!(error_name(&zbus::Error::InvalidReply), None);
    }

    /// Sin bus de sesión no se le pudo pedir nada a nadie: cada cuenta pedida
    /// vuelve como sin servicio, y la ventana lo dice al lado de cada una.
    #[test]
    fn sin_bus_cada_cuenta_pedida_queda_sin_servicio() {
        const VARIABLE: &str = "DBUS_SESSION_BUS_ADDRESS";
        let antes = std::env::var_os(VARIABLE);
        std::env::set_var(VARIABLE, "unix:path=/nonexistent/vasak-qa/bus");
        let ids = vec!["google".to_string(), "nube".to_string()];
        let fallos = tauri::async_runtime::block_on(request_sync(&ids));
        match antes {
            Some(valor) => std::env::set_var(VARIABLE, valor),
            None => std::env::remove_var(VARIABLE),
        }
        assert_eq!(
            fallos,
            vec![
                SyncRequestFailure {
                    account_id: "google".into(),
                    reason: "unavailable",
                },
                SyncRequestFailure {
                    account_id: "nube".into(),
                    reason: "unavailable",
                },
            ]
        );
    }

    // ── El aviso del estado ────────────────────────────────────────────────

    /// `StatusChanged` también sólo del sincronizador y de su objeto: si no,
    /// cualquier proceso de la sesión haría releer el estado a voluntad.
    #[test]
    fn solo_cuenta_el_status_changed_del_sincronizador_y_de_su_objeto() {
        let regla = status_rule().unwrap().to_string();
        for parte in [
            "type='signal'",
            "sender='ar.net.vasak.os.AccountsSync'",
            "path='/ar/net/vasak/os/AccountsStore'",
            "interface='ar.net.vasak.os.AccountsStore'",
            "member='StatusChanged'",
        ] {
            assert!(regla.contains(parte), "falta {parte} en {regla}");
        }
    }

    /// Una vuelta escribe de a lotes y avisa por cada uno: la ventana recibe
    /// un solo aviso por ráfaga, no uno por lote.
    #[test]
    fn una_rafaga_de_cambios_avisa_una_sola_vez() {
        assert_eq!(
            events_for(&[Notice::Changed, Notice::Changed, Notice::Changed]),
            [CHANGED_EVENT]
        );
    }

    /// Un cambio de estado solo no relee lo guardado, y un cambio de lo
    /// guardado solo no relee el estado.
    #[test]
    fn cada_clase_de_aviso_despierta_solo_lo_suyo() {
        assert_eq!(
            events_for(&[Notice::Status, Notice::Status]),
            [STATUS_EVENT]
        );
        assert_eq!(events_for(&[Notice::Changed]), [CHANGED_EVENT]);
    }

    #[test]
    fn una_rafaga_mixta_avisa_una_vez_de_cada_clase() {
        assert_eq!(
            events_for(&[
                Notice::Status,
                Notice::Changed,
                Notice::Status,
                Notice::Changed
            ]),
            [CHANGED_EVENT, STATUS_EVENT]
        );
        assert!(events_for(&[]).is_empty());
    }

    #[test]
    fn los_avisos_a_la_ventana_tienen_los_nombres_que_escucha() {
        assert_eq!(CHANGED_EVENT, EXPECTED_CHANGED_EVENT);
        assert_eq!(STATUS_EVENT, EXPECTED_STATUS_EVENT);
    }
}
