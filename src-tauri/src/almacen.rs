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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn card(id: &str, href: &str, vcard: &str) -> TarjetaGuardada {
        TarjetaGuardada {
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

    fn one_book() -> Vec<LibretaGuardada> {
        vec![LibretaGuardada {
            id: "1".into(),
            display_name: "Personal".into(),
        }]
    }

    /// Corre `leer` con páginas armadas de antemano y devuelve lo que leyó y
    /// los cursores con que se pidió cada página.
    async fn read_pages(
        books: Vec<LibretaGuardada>,
        pages: Vec<Result<String, String>>,
    ) -> (Result<Option<LecturaDeCuenta>, String>, Vec<String>) {
        let asked = RefCell::new(Vec::new());
        let pages = RefCell::new(pages.into_iter());
        let result = leer("cuenta", books, |cursor| {
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
        assert_eq!(
            identidad("cuenta", &t),
            "almacen:cuenta:https://x/a/ana.vcf"
        );
    }

    /// El número de la base puede cambiar entre vueltas; el `href` no. Con el
    /// mismo `href`, la ventana sigue mostrando al mismo contacto.
    #[test]
    fn la_identidad_no_cambia_si_cambia_el_numero_de_la_base() {
        let before = card("7", "https://x/a/ana.vcf", &named(1));
        let after = card("912", "https://x/a/ana.vcf", &named(1));
        assert_eq!(identidad("cuenta", &before), identidad("cuenta", &after));
    }

    #[test]
    fn sin_href_la_identidad_usa_el_numero_de_la_base() {
        let t = card("7", "", &named(1));
        assert_eq!(identidad("cuenta", &t), "almacen:cuenta:7");
        assert_ne!(
            identidad("cuenta", &t),
            identidad("cuenta", &card("8", "", &named(1)))
        );
    }

    /// Dos cuentas pueden tener la misma tarjeta en la misma dirección (dos
    /// personas en el mismo servidor, la misma libreta compartida): no son el
    /// mismo contacto para la ventana.
    #[test]
    fn la_misma_tarjeta_en_dos_cuentas_no_tiene_la_misma_identidad() {
        let t = card("7", "https://x/a/ana.vcf", &named(1));
        assert_ne!(identidad("una", &t), identidad("otra", &t));
        let bare = card("7", "", &named(1));
        assert_ne!(identidad("una", &bare), identidad("otra", &bare));
    }

    // ── Las libretas y las tarjetas ────────────────────────────────────────

    #[test]
    fn las_libretas_guardadas_pasan_a_la_forma_de_la_ventana() {
        let books = libretas_de(vec![
            LibretaGuardada {
                id: "3".into(),
                display_name: "Trabajo".into(),
            },
            LibretaGuardada {
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
        let contacts = contactos_de("cuenta", vec![stored.clone()]);
        assert_eq!(contacts.len(), 1);
        let ana = &contacts[0];

        // Lo mismo que daría el intérprete de la ventana, campo por campo.
        let expected =
            vcard::contact_from(raw, &identidad("cuenta", &stored)).expect("la tarjeta se lee");
        assert_eq!(ana, &expected);

        assert_eq!(ana.name, "Ana Pérez");
        assert_eq!(ana.url, "almacen:cuenta:https://x/a/ana.vcf");
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
        let contacts = contactos_de(
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
                ("Contacto 1", "almacen:cuenta:https://x/a/1.vcf"),
                ("Contacto 4", "almacen:cuenta:https://x/a/4.vcf"),
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
        let books: Vec<LibretaGuardada> = serde_json::from_str(json).unwrap();
        assert_eq!(
            books,
            vec![
                LibretaGuardada {
                    id: "3".into(),
                    display_name: "Trabajo".into(),
                },
                LibretaGuardada {
                    id: "1".into(),
                    display_name: "Personal".into(),
                },
            ]
        );
        let none: Vec<LibretaGuardada> = serde_json::from_str("[]").unwrap();
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
        assert_eq!(
            lectura.contactos[0].url,
            "almacen:cuenta:https://x/a/ana.vcf"
        );
        assert_eq!(
            lectura.contactos[0],
            vcard::contact_from(raw, "almacen:cuenta:https://x/a/ana.vcf").unwrap()
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
        let pages = (0..MAX_PAGINAS + 5)
            .map(|n| Ok(page_json(&[n], Some(&format!("c{n}")))))
            .collect();
        let (result, asked) = read_pages(one_book(), pages).await;
        let lectura = result.unwrap().unwrap();
        assert_eq!(asked.len(), MAX_PAGINAS);
        assert_eq!(lectura.contactos.len(), MAX_PAGINAS);
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
        zbus::Message::signal(RUTA, INTERFAZ, "Changed")
            .unwrap()
            .build(&(area, "cuenta", 3u64))
            .unwrap()
    }

    /// Sólo `Changed("contacts", …)` del almacén despierta a la ventana: un
    /// lote de correo o de calendario, otra señal o la misma señal de otra
    /// interfaz, no.
    #[test]
    fn la_regla_solo_deja_pasar_los_cambios_de_los_contactos() {
        let rule = regla_de_cambios().unwrap();
        assert!(rule.matches(&changed("contacts")).unwrap());
        assert!(!rule.matches(&changed("calendar")).unwrap());
        assert!(!rule.matches(&changed("email")).unwrap());

        let status = zbus::Message::signal(RUTA, INTERFAZ, "StatusChanged")
            .unwrap()
            .build(&())
            .unwrap();
        assert!(!rule.matches(&status).unwrap());

        let elsewhere = zbus::Message::signal(RUTA, "ar.net.vasak.os.Otra", "Changed")
            .unwrap()
            .build(&("contacts", "cuenta", 3u64))
            .unwrap();
        assert!(!rule.matches(&elsewhere).unwrap());

        let text = rule.to_string();
        assert!(text.contains("type='signal'"), "{text}");
        assert!(text.contains("arg0='contacts'"), "{text}");
    }
}
