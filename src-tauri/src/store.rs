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
//! Todo pasa por `tauri-plugin-vsk-store`, que comprueba que el dueño del
//! nombre del sincronizador sea el de verdad antes de creerle nada, pasa a la
//! ventana los avisos de lo nuevo y del estado, y atiende «Actualizar». Acá
//! queda lo propio de los contactos: qué se pide, el tope de tamaño de lo que
//! llega y cómo se lee cada tarjeta.

use serde::Deserialize;
use tauri_plugin_vsk_store::{Error as StoreError, StoreClient};

use crate::carddav::Libreta;
use crate::commands::LecturaDeCuenta;
use crate::vcard;

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

/// Llama a un método del almacén por el cliente comprobado del plugin, con
/// tope de tamaño en la respuesta. El error lleva el nombre del error de
/// D-Bus, que es lo que mira [`crate::commands::denied_by_store`].
async fn call<A>(store: &StoreClient, metodo: &str, argumentos: &A) -> Result<String, String>
where
    A: serde::ser::Serialize + zbus::zvariant::DynamicType,
{
    store
        .call_json(metodo, argumentos)
        .await
        .map_err(|e| describe(metodo, &e))
        .and_then(capped)
}

/// El mensaje de un error del plugin, con el nombre del error de D-Bus cuando
/// lo hay: el texto del plugin no lo lleva, y sin él
/// [`crate::commands::denied_by_store`] no reconoce un «no».
fn describe(metodo: &str, error: &StoreError) -> String {
    match error {
        StoreError::Store {
            name: Some(name), ..
        } => format!("{metodo}: {name}: {error}"),
        other => format!("{metodo}: {other}"),
    }
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
            // La libreta no sale de la tarjeta sino de dónde estaba: se la pone
            // acá, para que la ventana pueda filtrar por libreta (#4).
            let address_book_id = t.address_book_id.clone();
            vcard::split_cards(&t.vcard)
                .into_iter()
                .enumerate()
                .filter_map(move |(i, card)| {
                    let id = if i == 0 {
                        base.clone()
                    } else {
                        format!("{base}#{i}")
                    };
                    vcard::contact_from(&card, &id).map(|mut contact| {
                        contact.address_book_id = address_book_id.clone();
                        contact
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Lo que hay de una cuenta en el almacén. `Ok(None)` si el almacén todavía
/// no tiene libretas de esa cuenta —el área no se encendió, la primera vuelta
/// no terminó—: quien llama va al servidor como antes.
pub async fn read_contacts(
    store: &StoreClient,
    account_id: &str,
) -> Result<Option<LecturaDeCuenta>, String> {
    let json = call(store, "ListAddressBooks", &(account_id,)).await?;
    let libretas: Vec<StoredAddressBook> = serde_json::from_str(&json)
        .map_err(|e| format!("no se pudieron leer las libretas: {e}"))?;
    let llamada = |cursor: String| async move {
        call(
            store,
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

        // Lo mismo que daría el intérprete de la ventana, campo por campo, más
        // la libreta, que no sale de la tarjeta sino de dónde estaba.
        let mut expected =
            vcard::contact_from(raw, &identity("cuenta", &stored)).expect("la tarjeta se lee");
        expected.address_book_id = "1".into();
        assert_eq!(ana, &expected);
        // La libreta de la tarjeta viaja con el contacto: es lo que deja filtrar
        // por libreta (#4).
        assert_eq!(ana.address_book_id, "1");

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

    /// Cada contacto sale con la libreta en la que vive —el `address_book_id`
    /// de su tarjeta—, que es lo que deja elegir qué libretas se ven (#4). Dos
    /// tarjetas del mismo recurso llevan la misma, y una sin libreta sale
    /// vacía.
    #[test]
    fn cada_contacto_sale_con_la_libreta_de_su_tarjeta() {
        let personal = StoredCard {
            id: "1".into(),
            address_book_id: "personal".into(),
            href: "https://x/a/1.vcf".into(),
            vcard: named(1),
        };
        let trabajo = StoredCard {
            id: "2".into(),
            address_book_id: "trabajo".into(),
            href: "https://x/b/2.vcf".into(),
            vcard: named(2),
        };
        let sin_libreta = StoredCard {
            id: "3".into(),
            address_book_id: String::new(),
            href: "https://x/c/3.vcf".into(),
            vcard: named(3),
        };

        let contacts = contacts_from("cuenta", vec![personal, trabajo, sin_libreta]);
        let libretas: Vec<&str> = contacts
            .iter()
            .map(|c| c.address_book_id.as_str())
            .collect();
        assert_eq!(libretas, vec!["personal", "trabajo", ""]);
    }

    /// Un recurso con dos tarjetas: las dos quedan con la libreta del recurso.
    #[test]
    fn dos_tarjetas_de_un_recurso_comparten_su_libreta() {
        let two = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\nEND:VCARD\r\n\
                   BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bruno\r\nEND:VCARD\r\n";
        let stored = StoredCard {
            id: "7".into(),
            address_book_id: "compartida".into(),
            href: "https://x/todos.vcf".into(),
            vcard: two.into(),
        };
        let contacts = contacts_from("a1", vec![stored]);
        assert_eq!(contacts.len(), 2);
        assert!(contacts.iter().all(|c| c.address_book_id == "compartida"));
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
        let mut expected = vcard::contact_from(raw, "store:cuenta:https://x/a/ana.vcf").unwrap();
        expected.address_book_id = "1".into();
        assert_eq!(lectura.contactos[0], expected);
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
    // ── Los errores del plugin ─────────────────────────────────────────────

    fn store_error(name: Option<&str>, message: &str) -> StoreError {
        StoreError::Store {
            name: name.map(str::to_string),
            message: message.into(),
        }
    }

    /// El texto de un error del plugin no lleva el nombre del error de D-Bus;
    /// el mensaje tiene que llevarlo, o un «no» de la persona se lee como un
    /// almacén caído y se le pide al servidor el permiso más fuerte.
    #[test]
    fn un_no_del_plugin_se_sigue_leyendo_como_un_no() {
        let error = store_error(
            Some("org.freedesktop.DBus.Error.AccessDenied"),
            "sin permiso",
        );
        let mensaje = describe("ListAddressBooks", &error);
        assert_eq!(
            mensaje,
            "ListAddressBooks: org.freedesktop.DBus.Error.AccessDenied: sin permiso"
        );
        assert!(crate::commands::denied_by_store(&mensaje));
    }

    #[test]
    fn los_demas_errores_del_plugin_no_son_un_no() {
        for error in [
            store_error(Some("org.freedesktop.DBus.Error.ServiceUnknown"), "no está"),
            store_error(Some("org.freedesktop.DBus.Error.Failed"), "falló"),
            store_error(None, "sin nombre"),
            StoreError::NotRunning,
            StoreError::Impostor,
            StoreError::Reply("rota".into()),
        ] {
            let mensaje = describe("ListContactCards", &error);
            assert!(mensaje.starts_with("ListContactCards: "), "{mensaje}");
            assert!(!crate::commands::denied_by_store(&mensaje), "{mensaje}");
        }
    }

    // ── Por el cliente comprobado del plugin ───────────────────────────────

    use crate::test_bus::{self, Calls, PrivateBus};

    /// El almacén de mentira en el bus privado: anota cada pedido con sus
    /// argumentos. Con `deny`, contesta que la persona no dio permiso.
    #[derive(Clone, Default)]
    struct FakeStore {
        deny: bool,
        calls: Calls,
    }

    #[zbus::interface(name = "ar.net.vasak.os.AccountsStore")]
    impl FakeStore {
        fn list_address_books(&self, account_id: String) -> zbus::fdo::Result<String> {
            self.calls.push(format!("ListAddressBooks {account_id}"));
            if self.deny {
                return Err(zbus::fdo::Error::AccessDenied("sin permiso".into()));
            }
            Ok(serde_json::json!([{ "id": "1", "display_name": "Personal" }]).to_string())
        }

        fn list_contact_cards(
            &self,
            account_id: String,
            address_book_id: String,
            cursor: String,
            limit: u32,
        ) -> String {
            self.calls.push(format!(
                "ListContactCards {account_id} {address_book_id:?} {cursor:?} {limit}"
            ));
            if cursor.is_empty() {
                page_json(&[1], Some("2"))
            } else {
                page_json(&[2], None)
            }
        }
    }

    /// Los contactos salen del sincronizador comprobado, de la cuenta pedida,
    /// de todas sus libretas y página por página.
    #[tokio::test]
    async fn los_contactos_se_leen_del_almacen_por_el_cliente_del_plugin() {
        let _turno = test_bus::exclusive();
        let bus = PrivateBus::start();
        let store = FakeStore::default();
        let calls = store.calls.clone();
        let _servicio = bus.serve(tauri_plugin_vsk_store::client::PATH, store).await;

        let lectura = read_contacts(&test_bus::client(true), "nube")
            .await
            .unwrap()
            .expect("había libretas");

        assert_eq!(names(&lectura), ["Contacto 1", "Contacto 2"]);
        assert_eq!(lectura.libretas[0].nombre, "Personal");
        assert_eq!(
            calls.taken(),
            [
                "ListAddressBooks nube".to_string(),
                format!("ListContactCards nube \"\" \"\" {PAGE_SIZE}"),
                format!("ListContactCards nube \"\" \"2\" {PAGE_SIZE}"),
            ]
        );
    }

    /// Un `AccessDenied` de verdad, por el bus y por el plugin, llega a la
    /// ventana como un «no»: no se va al servidor a pedir más.
    #[tokio::test]
    async fn un_no_del_almacen_por_el_bus_no_lleva_al_servidor() {
        let _turno = test_bus::exclusive();
        let bus = PrivateBus::start();
        let store = FakeStore {
            deny: true,
            ..Default::default()
        };
        let _servicio = bus.serve(tauri_plugin_vsk_store::client::PATH, store).await;

        let error = read_contacts(&test_bus::client(true), "nube")
            .await
            .unwrap_err();

        assert!(error.starts_with("ListAddressBooks: "), "{error}");
        assert!(crate::commands::denied_by_store(&error), "{error}");
    }

    /// A un impostor no se le pide nada, y su rechazo no se lee como un «no»
    /// de la persona: la ventana va al servidor como con el almacén caído.
    #[tokio::test]
    async fn a_un_impostor_no_se_le_piden_los_contactos() {
        let _turno = test_bus::exclusive();
        let bus = PrivateBus::start();
        let store = FakeStore::default();
        let calls = store.calls.clone();
        let _impostor = bus.serve(tauri_plugin_vsk_store::client::PATH, store).await;

        let error = read_contacts(&test_bus::client(false), "nube")
            .await
            .unwrap_err();

        assert!(calls.taken().is_empty(), "{:?}", calls.taken());
        assert!(!crate::commands::denied_by_store(&error), "{error}");
    }
}
