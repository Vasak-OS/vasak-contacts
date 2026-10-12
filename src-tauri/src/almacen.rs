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
//! como [`EVENTO`]; el botón «Actualizar» pide `RequestSync`.
//!
//! La primera lectura puede abrir el diálogo de `vasak-permissions`
//! (`store.contacts`): por eso las llamadas no tienen tope de tiempo.

use serde::Deserialize;

use crate::carddav::Libreta;
use crate::comandos::LecturaDeCuenta;
use crate::vcard;

const SERVICIO: &str = "ar.net.vasak.os.AccountsSync";
const RUTA: &str = "/ar/net/vasak/os/AccountsStore";
const INTERFAZ: &str = "ar.net.vasak.os.AccountsStore";

/// El aviso a la ventana de que cambió lo guardado de los contactos.
pub const EVENTO: &str = "contactos-cambio";

/// Cuántas tarjetas se piden por página: el máximo que acepta el almacén.
const PAGINA: u32 = 1000;

/// Cuántas páginas se leen como mucho por cuenta. Cincuenta mil contactos no
/// es una agenda personal; se muestra lo que entró y se avisa.
const MAX_PAGINAS: usize = 50;

/// Una libreta como la lista el almacén.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LibretaGuardada {
    pub id: String,
    pub display_name: String,
}

/// Una tarjeta cruda del almacén.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TarjetaGuardada {
    pub id: String,
    #[serde(default)]
    pub address_book_id: String,
    #[serde(default)]
    pub href: String,
    pub vcard: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PaginaDeTarjetas {
    items: Vec<TarjetaGuardada>,
    #[serde(default)]
    next_cursor: Option<String>,
}

async fn conectar() -> Result<zbus::Connection, String> {
    zbus::Connection::session()
        .await
        .map_err(|e| format!("no se pudo contactar al almacén del sistema: {e}"))
}

async fn llamar<A>(metodo: &str, argumentos: &A) -> Result<String, String>
where
    A: serde::ser::Serialize + zbus::zvariant::DynamicType,
{
    conectar()
        .await?
        .call_method(Some(SERVICIO), RUTA, Some(INTERFAZ), metodo, argumentos)
        .await
        .map_err(|e| format!("{metodo}: {e}"))?
        .body()
        .deserialize()
        .map_err(|e| format!("respuesta inválida de {metodo}: {e}"))
}

/// La identidad de un contacto del almacén: la cuenta y dónde vive la
/// tarjeta en el servidor. Es lo que la ventana compara para seguir mostrando
/// el mismo contacto después de releer, así que no puede cambiar entre vueltas
/// —el número de la base sí podría— ni repetirse entre cuentas.
pub fn identidad(account_id: &str, tarjeta: &TarjetaGuardada) -> String {
    let donde = if tarjeta.href.is_empty() {
        &tarjeta.id
    } else {
        &tarjeta.href
    };
    format!("almacen:{account_id}:{donde}")
}

/// Las libretas en la forma de la ventana.
pub fn libretas_de(guardadas: Vec<LibretaGuardada>) -> Vec<Libreta> {
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
pub fn contactos_de(account_id: &str, tarjetas: Vec<TarjetaGuardada>) -> Vec<vcard::Contact> {
    tarjetas
        .into_iter()
        .filter_map(|t| vcard::contact_from(&t.vcard, &identidad(account_id, &t)))
        .collect()
}

/// Lo que hay de una cuenta en el almacén. `Ok(None)` si el almacén todavía
/// no tiene libretas de esa cuenta —el área no se encendió, la primera vuelta
/// no terminó—: quien llama va al servidor como antes.
pub async fn contactos(account_id: &str) -> Result<Option<LecturaDeCuenta>, String> {
    let json = llamar("ListAddressBooks", &(account_id,)).await?;
    let libretas: Vec<LibretaGuardada> = serde_json::from_str(&json)
        .map_err(|e| format!("no se pudieron leer las libretas: {e}"))?;
    let llamada = |cursor: String| async move {
        llamar(
            "ListContactCards",
            &(account_id, "", cursor.as_str(), PAGINA),
        )
        .await
    };
    leer(account_id, libretas, llamada).await
}

/// Lo de [`contactos`] sin el bus: las libretas ya leídas, y cómo pedir cada
/// página.
pub async fn leer<F, Fut>(
    account_id: &str,
    libretas: Vec<LibretaGuardada>,
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
        libretas: libretas_de(libretas),
        ..Default::default()
    };
    let mut cursor = String::new();
    for _ in 0..MAX_PAGINAS {
        let json = pagina(cursor.clone()).await?;
        let pagina: PaginaDeTarjetas = serde_json::from_str(&json)
            .map_err(|e| format!("no se pudieron leer los contactos: {e}"))?;
        lectura
            .contactos
            .extend(contactos_de(account_id, pagina.items));
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

/// Le pide al sincronizador una vuelta ya de unas cuentas: el botón
/// «Actualizar». No espera a que termine: lo nuevo se avisa con [`EVENTO`].
pub async fn pedir_sincronizacion(account_ids: &[String]) {
    let Ok(conexion) = conectar().await else {
        return;
    };
    for id in account_ids {
        if let Err(e) = conexion
            .call_method(
                Some(SERVICIO),
                RUTA,
                Some(INTERFAZ),
                "RequestSync",
                &(id.as_str(),),
            )
            .await
        {
            eprintln!("[contactos] no se pudo pedir una vuelta de una cuenta: {e}");
        }
    }
}

/// La regla de la señal que interesa: `Changed` del almacén, sólo de los
/// contactos. El bus filtra por el primer argumento, así que un lote de correo
/// o de calendario no despierta a esta ventana.
pub fn regla_de_cambios() -> zbus::Result<zbus::MatchRule<'static>> {
    Ok(zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(INTERFAZ)?
        .member("Changed")?
        .arg(0, "contacts")?
        .build())
}

/// Pasa a la ventana cada `Changed("contacts", …)` del almacén.
pub fn escuchar(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // En bucle: el bus se puede cortar, y con un solo intento la ventana
        // dejaría de enterarse de lo nuevo hasta que se la cierre.
        loop {
            if let Err(e) = seguir(&app).await {
                eprintln!("[contactos] no se pueden recibir avisos del almacén: {e}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

async fn seguir(app: &tauri::AppHandle) -> Result<(), String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let conexion = conectar().await?;
    let regla = regla_de_cambios().map_err(|e| format!("no se pudo armar el filtro: {e}"))?;
    let mut avisos = zbus::MessageStream::for_match_rule(regla, &conexion, None)
        .await
        .map_err(|e| format!("no se pudo escuchar al almacén: {e}"))?;
    while let Some(aviso) = avisos.next().await {
        if aviso.is_ok() {
            let _ = app.emit(EVENTO, ());
        }
    }
    Err("el bus de sesión cerró la conexión".into())
}
