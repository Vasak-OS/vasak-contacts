//! Tarjetas enteras como las exportan Google, iCloud y Nextcloud.
//!
//! **Son reconstrucciones**, armadas con la forma que tiene cada exportador
//! —los grupos `itemN.` y el año 1604 de Apple, los dos puntos escapados y las
//! categorías del sistema de Google, la 4.0 con `data:` URI y `VALUE=uri` de
//! Nextcloud— y con datos inventados. Lo que se prueba es que cada rareza de
//! cada uno llegue a la ficha, no el contenido.
//!
//! Y todas se prueban también **plegadas a 75 octetos**, que es como viajan: un
//! contacto que se lee bien sin plegar y mal plegado es uno que se ve roto en
//! la libreta de verdad.

use base64::Engine;

use super::contact::*;
use super::date::PartialDate;
use super::photo::tests::{jpeg_of, PNG_1X1};
use super::photo::{PhotoSkipped, MAX_PHOTO_BYTES};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Pliega cada renglón a 75 octetos, como el estándar: corta sin partir un
/// carácter y sigue en el renglón siguiente con un espacio adelante.
fn fold(card: &str) -> String {
    let mut out = String::new();
    for line in card.split("\r\n") {
        let mut rest = line;
        let mut first = true;
        while !rest.is_empty() {
            let budget = if first { 75 } else { 74 };
            let mut cut = budget.min(rest.len());
            while !rest.is_char_boundary(cut) {
                cut -= 1;
            }
            if !first {
                out.push(' ');
            }
            out.push_str(&rest[..cut]);
            out.push_str("\r\n");
            rest = &rest[cut..];
            first = false;
        }
    }
    out
}

fn ymd(year: Option<i32>, month: u8, day: u8) -> Option<PartialDate> {
    Some(PartialDate {
        year,
        month: Some(month),
        day: Some(day),
        text: String::new(),
    })
}

/// La forma de Google: 3.0, los dos puntos de las direcciones escapados, la
/// foto como dirección externa, el cumpleaños sin año con guiones, y sus dos
/// grupos del sistema entre las categorías.
fn google() -> String {
    [
        "BEGIN:VCARD",
        "VERSION:3.0",
        "FN:Ana Pérez",
        "N:Pérez;Ana;;;",
        "NICKNAME:Anita",
        "EMAIL;TYPE=INTERNET;TYPE=HOME:ana@gmail.com",
        "EMAIL;TYPE=INTERNET;TYPE=WORK:ana@vasak.net.ar",
        "TEL;TYPE=CELL:+54 9 11 5555-1234",
        "ADR;TYPE=HOME:;;Av. Siempreviva 742;Springfield;Buenos Aires;B1636;Argentina",
        "ORG:Vasak Group",
        "TITLE:Jefa de soporte",
        "BDAY:--04-15",
        r"item1.URL:https\://ana.ejemplo.com",
        "item1.X-ABLabel:BLOG",
        "item2.X-ABDATE:2010-06-12",
        "item2.X-ABLabel:_$!<Anniversary>!$_",
        "item3.X-ABRELATEDNAMES:Juan Pérez",
        "item3.X-ABLabel:_$!<Spouse>!$_",
        "PHOTO:https://lh3.googleusercontent.com/contacts/ABCdef123",
        "CATEGORIES:myContacts,starred,Amigos",
        r"NOTE:Le gusta el mate\, sin azúcar.",
        "END:VCARD",
    ]
    .join("\r\n")
        + "\r\n"
}

/// La forma de iCloud: 3.0, los grupos `itemN.` con su `X-ABLabel`, el año
/// 1604 para decir «sin año», la foto adentro en base64, el perfil de Twitter
/// con el usuario en `x-user`, y `IMPP` con el servicio en `X-SERVICE-TYPE`.
fn icloud() -> String {
    [
        "BEGIN:VCARD".to_string(),
        "VERSION:3.0".into(),
        "PRODID:-//Apple Inc.//iPhone OS 17.5//EN".into(),
        "N:Pérez;Ana;;;".into(),
        "FN:Ana Pérez".into(),
        "NICKNAME:Anita".into(),
        "ORG:Vasak Group;Soporte;".into(),
        "TITLE:Jefa de soporte".into(),
        "item1.EMAIL;type=INTERNET;type=pref:ana@icloud.com".into(),
        "item1.X-ABLabel:_$!<Other>!$_".into(),
        "TEL;type=CELL;type=VOICE;type=pref:+54 9 11 5555-1234".into(),
        "item2.ADR;type=HOME;type=pref:;;Av. Siempreviva 742;Springfield;Buenos Aires;B1636;Argentina".into(),
        "item2.X-ABADR:ar".into(),
        "item3.URL;type=pref:https://ana.ejemplo.com".into(),
        "item3.X-ABLabel:_$!<HomePage>!$_".into(),
        "BDAY;X-APPLE-OMIT-YEAR=1604:1604-04-15".into(),
        "item4.X-ABDATE;type=pref:2010-06-12".into(),
        "item4.X-ABLabel:_$!<Anniversary>!$_".into(),
        "item5.X-ABDATE:2015-03-01".into(),
        "item5.X-ABLabel:Llegó a la oficina".into(),
        "X-SOCIALPROFILE;type=twitter;x-user=anaperez:http://twitter.com/anaperez".into(),
        "IMPP;X-SERVICE-TYPE=Skype;type=HOME;type=pref:skype:ana.perez".into(),
        "item6.X-ABRELATEDNAMES;type=pref:Juan Pérez".into(),
        "item6.X-ABLabel:_$!<Spouse>!$_".into(),
        "X-ABUID:5D4A7C1E-0000-4000-8000-000000000000:ABPerson".into(),
        "X-PHONETIC-FIRST-NAME:A-na".into(),
        format!("PHOTO;ENCODING=b;TYPE=JPEG:{}", b64(&jpeg_of(2048))),
        "END:VCARD".into(),
    ]
    .join("\r\n")
        + "\r\n"
}

/// La forma de Nextcloud en 4.0: `data:` URI, `VALUE=uri`, el `TYPE` entre
/// comillas con dos valores, la dirección con un `LABEL` que tiene dos puntos
/// adentro, y las propiedades que sólo existen en la 4.0.
fn nextcloud() -> String {
    [
        "BEGIN:VCARD".to_string(),
        "VERSION:4.0".into(),
        "PRODID:-//Sabre//Sabre VObject 4.5.4//EN".into(),
        "UID:7b2c9d10-5c1a-4e5e-9a61-2f1d7c0a9b11".into(),
        "FN:Ana Pérez".into(),
        "N:Pérez;Ana;;;".into(),
        "KIND:individual".into(),
        "EMAIL;TYPE=work:ana@vasak.net.ar".into(),
        "TEL;TYPE=\"cell,voice\";VALUE=uri:tel:+54-9-11-5555-1234".into(),
        "ADR;TYPE=work;LABEL=\"Piso 3: oficina B\":;;Av. Siempreviva 742;Springfield;;B1636;Argentina".into(),
        "BDAY:--0415".into(),
        "ANNIVERSARY:20100612".into(),
        "URL;VALUE=uri:https://ana.ejemplo.com".into(),
        "IMPP;PREF=1:xmpp:ana@jabber.org".into(),
        "X-SOCIALPROFILE;TYPE=mastodon:@ana@mastodon.social".into(),
        "LANG;PREF=1:es-AR".into(),
        "LANG;PREF=2:en".into(),
        "TZ;VALUE=utc-offset:-0300".into(),
        "GEO:geo:-34.6037,-58.3816".into(),
        "CATEGORIES:Trabajo,Amigos".into(),
        "TITLE:Jefa de soporte".into(),
        "ROLE:Programadora".into(),
        "RELATED;TYPE=spouse;VALUE=text:Juan Pérez".into(),
        format!("PHOTO:data:image/png;base64,{}", b64(PNG_1X1)),
        "END:VCARD".into(),
    ]
    .join("\r\n")
        + "\r\n"
}

/// Cada tarjeta, sin plegar y plegada: tienen que dar lo mismo.
fn both(card: String) -> Contact {
    let flat = contact_from(&card, "").expect("la tarjeta tiene contacto");
    let folded = contact_from(&fold(&card), "").expect("plegada también");
    assert_eq!(flat, folded, "plegada se lee distinto");
    flat
}

#[test]
fn el_plegado_de_prueba_pliega_de_verdad() {
    // Si `fold` no plegara, las pruebas de «plegada da lo mismo» no probarían
    // nada.
    let folded = fold(&icloud());
    assert!(folded.lines().all(|l| l.len() <= 75));
    assert!(folded.lines().filter(|l| l.starts_with(' ')).count() > 20);
}

#[test]
fn la_tarjeta_de_google_se_lee_entera() {
    let c = both(google());

    assert_eq!(c.name, "Ana Pérez");
    assert_eq!(c.nickname, "Anita");
    assert_eq!(c.title, "Jefa de soporte");
    assert_eq!(c.birthday, ymd(None, 4, 15));
    // La fecha de Apple que usa Google, con la etiqueta de aniversario.
    assert_eq!(c.anniversary, ymd(Some(2010), 6, 12));
    // La dirección con los dos puntos escapados, y la etiqueta del grupo.
    assert_eq!(
        c.websites,
        vec![LabeledValue {
            label: "blog".into(),
            value: "https://ana.ejemplo.com".into()
        }]
    );
    // La foto no se baja al leer: queda la dirección para que la baje el
    // programa.
    assert_eq!(c.photo, "");
    assert_eq!(
        c.photo_url,
        "https://lh3.googleusercontent.com/contacts/ABCdef123"
    );
    // Los grupos del sistema de Google no son categorías de la persona.
    assert_eq!(c.categories, vec!["Amigos"]);
    assert_eq!(c.notes, "Le gusta el mate, sin azúcar.");
    assert_eq!(c.addresses.len(), 1);
    assert_eq!(c.addresses[0].label, "home");
    assert_eq!(c.addresses[0].street, "Av. Siempreviva 742");
    assert_eq!(c.addresses[0].country, "Argentina");
    // La relación es del #6: no se cuela como campo a medida.
    assert!(c.custom_fields.is_empty(), "{:?}", c.custom_fields);
}

#[test]
fn la_tarjeta_de_icloud_se_lee_entera() {
    let c = both(icloud());

    assert_eq!(c.organization, "Vasak Group, Soporte");
    // La etiqueta de Apple tapa al `TYPE`.
    assert_eq!(c.emails[0].label, "other");
    assert_eq!(c.phones[0].label, "cell");
    // El año 1604 no es un año.
    assert_eq!(c.birthday, ymd(None, 4, 15));
    assert_eq!(c.anniversary, ymd(Some(2010), 6, 12));
    // Y la fecha con una etiqueta que escribió la persona.
    assert_eq!(c.other_dates.len(), 1);
    assert_eq!(c.other_dates[0].label, "llegó a la oficina");
    assert_eq!(c.other_dates[0].date, ymd(Some(2015), 3, 1).unwrap());

    assert_eq!(c.websites[0].label, "homepage");
    assert_eq!(c.websites[0].value, "https://ana.ejemplo.com");

    let twitter = c.social.iter().find(|s| s.service == "twitter").unwrap();
    assert_eq!(twitter.handle, "anaperez");
    assert_eq!(twitter.url, "http://twitter.com/anaperez");
    let skype = c.social.iter().find(|s| s.service == "skype").unwrap();
    assert_eq!(skype.handle, "ana.perez");
    assert_eq!(skype.label, "home");

    // La foto vino adentro y está lista para mostrar.
    assert!(
        c.photo.starts_with("data:image/jpeg;base64,"),
        "{}",
        &c.photo[..40.min(c.photo.len())]
    );
    assert_eq!(c.photo_url, "");
    assert_eq!(c.photo_skipped, None);

    // Lo interno de Apple no se muestra; lo que escribió la persona, sí.
    assert_eq!(
        c.custom_fields,
        vec![CustomField {
            name: "X-PHONETIC-FIRST-NAME".into(),
            label: String::new(),
            value: "A-na".into()
        }]
    );
    assert_eq!(c.addresses[0].locality, "Springfield");
}

#[test]
fn la_tarjeta_de_nextcloud_en_40_se_lee_entera() {
    let c = both(nextcloud());

    assert_eq!(c.uid, "7b2c9d10-5c1a-4e5e-9a61-2f1d7c0a9b11");
    assert_eq!(c.kind, "individual");
    assert_eq!(c.phones[0].value, "+54-9-11-5555-1234");
    assert_eq!(c.phones[0].label, "cell");
    // El `LABEL` de la dirección, con sus dos puntos adentro, no parte nada y
    // no se muestra como etiqueta.
    assert_eq!(c.addresses[0].label, "work");
    assert_eq!(c.addresses[0].street, "Av. Siempreviva 742");
    assert_eq!(c.addresses[0].region, "");
    assert_eq!(c.addresses[0].postal_code, "B1636");
    assert_eq!(c.birthday, ymd(None, 4, 15));
    assert_eq!(c.anniversary, ymd(Some(2010), 6, 12));
    assert_eq!(c.websites[0].value, "https://ana.ejemplo.com");
    assert_eq!(c.languages, vec!["es-AR", "en"]);
    assert_eq!(c.time_zone, "-0300");
    assert_eq!(c.geo, "-34.6037, -58.3816");
    assert_eq!(c.categories, vec!["Trabajo", "Amigos"]);
    assert_eq!(c.title, "Jefa de soporte");
    assert_eq!(c.role, "Programadora");
    assert!(c.photo.starts_with("data:image/png;base64,"));

    let xmpp = c.social.iter().find(|s| s.service == "xmpp").unwrap();
    assert_eq!(xmpp.handle, "ana@jabber.org");
    let mastodon = c.social.iter().find(|s| s.service == "mastodon").unwrap();
    assert_eq!(mastodon.handle, "@ana@mastodon.social");
    // `RELATED` es del #6: ni se muestra ni se cuela.
    assert!(c.custom_fields.is_empty());
}

/// **Una foto enorme no se manda a la ventana**, pero el resto de la tarjeta
/// se lee igual, y se dice por qué no hay foto.
#[test]
fn una_foto_enorme_no_se_muestra_y_el_resto_se_lee() {
    let card = format!(
        "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\nPHOTO;ENCODING=b;TYPE=JPEG:{}\r\nEMAIL:ana@x.com\r\nEND:VCARD\r\n",
        b64(&jpeg_of(MAX_PHOTO_BYTES * 2))
    );
    let c = both(card);
    assert_eq!(c.photo, "");
    assert_eq!(c.photo_skipped, Some(PhotoSkipped::TooLarge));
    assert_eq!(c.emails[0].value, "ana@x.com");
}

/// Una foto que no sirve no tapa a la siguiente que sí.
#[test]
fn una_foto_que_no_sirve_no_tapa_a_la_siguiente() {
    let card = format!(
        "BEGIN:VCARD\r\nFN:Ana\r\nPHOTO;VALUE=uri:http://inseguro.ejemplo.com/a.jpg\r\nPHOTO;ENCODING=b:{}\r\nEND:VCARD",
        b64(PNG_1X1)
    );
    let c = contact_from(&card, "").unwrap();
    assert!(c.photo.starts_with("data:image/png"));
    assert_eq!(c.photo_skipped, None);

    // Y si ninguna sirve, queda el primer motivo.
    let card = "BEGIN:VCARD\r\nFN:Ana\r\nPHOTO;VALUE=uri:http://x/a.jpg\r\nEND:VCARD";
    assert_eq!(
        contact_from(card, "").unwrap().photo_skipped,
        Some(PhotoSkipped::Insecure)
    );
}

/// **La dirección con campos vacíos**: los que faltan quedan vacíos y la
/// dirección se muestra con lo que tiene. Y una sin nada no es una dirección.
#[test]
fn una_direccion_con_campos_vacios() {
    let card = "BEGIN:VCARD\r\nFN:Ana\r\n\
        ADR;TYPE=HOME:;;;Springfield;;;\r\n\
        ADR;TYPE=WORK:;;;;;;\r\n\
        ADR:;;Calle sin ciudad\r\n\
        ADR;TYPE=HOME:;;Av. Rivadavia 1234\\; 3° B;CABA;;C1033;\r\n\
        END:VCARD";
    let c = contact_from(card, "").unwrap();

    assert_eq!(c.addresses.len(), 3, "{:?}", c.addresses);
    assert_eq!(c.addresses[0].locality, "Springfield");
    assert_eq!(c.addresses[0].street, "");
    // Con menos de siete campos, los que faltan quedan vacíos en vez de
    // correrse.
    assert_eq!(c.addresses[1].street, "Calle sin ciudad");
    assert_eq!(c.addresses[1].country, "");
    // El punto y coma escapado es parte de la calle.
    assert_eq!(c.addresses[2].street, "Av. Rivadavia 1234; 3° B");
    assert_eq!(c.addresses[2].postal_code, "C1033");
}

/// La 2.1 manda la dirección en `quoted-printable` cuando tiene acentos, y se
/// decodifica antes de partirla.
#[test]
fn una_direccion_de_la_21_con_acentos() {
    let card = "BEGIN:VCARD\r\nVERSION:2.1\r\nFN:Ana\r\n\
        ADR;HOME;ENCODING=QUOTED-PRINTABLE:;;Av. Mayo 1=C2=BA;Bogot=C3=A1;;;Colombia\r\n\
        END:VCARD";
    let c = contact_from(card, "").unwrap();
    assert_eq!(c.addresses[0].label, "home");
    assert_eq!(c.addresses[0].street, "Av. Mayo 1º");
    assert_eq!(c.addresses[0].locality, "Bogotá");
}

/// El cargo, la función, el apodo, y qué es la tarjeta.
#[test]
fn el_cargo_el_apodo_y_el_tipo_de_tarjeta() {
    let card = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Soporte Vasak\r\n\
        TITLE:Mesa de ayuda\r\nROLE:Atención\r\nNICKNAME:Sopo,La mesa\r\n\
        KIND:org\r\nEND:VCARD";
    let c = contact_from(card, "").unwrap();
    assert_eq!(c.title, "Mesa de ayuda");
    assert_eq!(c.role, "Atención");
    assert_eq!(c.nickname, "Sopo, La mesa");
    assert_eq!(c.kind, "org");

    // Apple lo dice con `X-ABShowAs`, y un `KIND` inventado no se guarda.
    let apple = "BEGIN:VCARD\r\nFN:Vasak\r\nX-ABShowAs:COMPANY\r\nEND:VCARD";
    assert_eq!(contact_from(apple, "").unwrap().kind, "org");
    let inventado = "BEGIN:VCARD\r\nFN:Vasak\r\nKIND:marciano\r\nEND:VCARD";
    assert_eq!(contact_from(inventado, "").unwrap().kind, "");
}

/// Los `X-` de redes que escriben Evolution y KDE.
#[test]
fn las_redes_de_evolution_y_kde() {
    let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\n\
        X-SKYPE;TYPE=HOME:ana.perez\r\nX-JABBER:ana@jabber.org\r\n\
        X-TWITTER:@anaperez\r\nEND:VCARD";
    let c = contact_from(card, "").unwrap();
    let services: Vec<&str> = c.social.iter().map(|s| s.service.as_str()).collect();
    assert_eq!(services, vec!["skype", "xmpp", "twitter"]);
    assert_eq!(c.social[0].label, "home");
    // Ninguno termina, además, entre los campos a medida.
    assert!(c.custom_fields.is_empty());
}

/// **Los campos a medida**: los `X-` que no son de nadie se conservan y se
/// muestran, con la etiqueta de Apple si la tienen. Hoy se tiraban.
#[test]
fn los_campos_a_medida_se_conservan() {
    let card = "BEGIN:VCARD\r\nFN:Ana\r\n\
        X-NUMERO-DE-SOCIO:4521\r\n\
        item1.X-CUSTOM:azul\r\nitem1.X-ABLabel:Color favorito\r\n\
        X-VACIO:\r\n\
        X-MOZILLA-HTML:FALSE\r\n\
        END:VCARD";
    let c = contact_from(card, "").unwrap();
    assert_eq!(
        c.custom_fields,
        vec![
            CustomField {
                name: "X-NUMERO-DE-SOCIO".into(),
                label: String::new(),
                value: "4521".into()
            },
            CustomField {
                name: "X-CUSTOM".into(),
                label: "color favorito".into(),
                value: "azul".into()
            },
        ]
    );
}

/// Una tarjeta con quinientas categorías o quinientos campos a medida no es
/// una tarjeta que se pueda leer: se guardan las primeras.
#[test]
fn las_listas_tienen_tope() {
    let muchas: Vec<String> = (0..500).map(|i| format!("cat{i}")).collect();
    let card = format!(
        "BEGIN:VCARD\r\nFN:Ana\r\nCATEGORIES:{}\r\n{}END:VCARD",
        muchas.join(","),
        (0..500)
            .map(|i| format!("X-CAMPO-{i}:v\r\n"))
            .collect::<String>()
    );
    let c = contact_from(&card, "").unwrap();
    assert_eq!(c.categories.len(), MAX_LIST_ITEMS);
    assert_eq!(c.custom_fields.len(), MAX_LIST_ITEMS);
}

/// La ubicación, de las dos versiones, y una que no se entiende no se muestra.
#[test]
fn la_ubicacion_de_las_dos_versiones() {
    let geo = |value: &str| {
        let card = format!("BEGIN:VCARD\r\nFN:Ana\r\nGEO:{value}\r\nEND:VCARD");
        contact_from(&card, "").unwrap().geo
    };
    assert_eq!(geo("37.386013;-122.082932"), "37.386013, -122.082932");
    assert_eq!(geo("geo:37.386013,-122.082932"), "37.386013, -122.082932");
    assert_eq!(geo("geo:37.38,-122.08;u=35"), "37.38, -122.08");
    assert_eq!(geo("geo:200,10"), "");
    assert_eq!(geo("en casa"), "");
    assert_eq!(geo("NaN;NaN"), "");
}

/// Un idioma es una etiqueta corta: lo que no lo parece no se guarda, y no se
/// repite.
#[test]
fn los_idiomas() {
    let card = "BEGIN:VCARD\r\nFN:Ana\r\nLANG:es-AR\r\nLANG:es-AR\r\n\
        LANG:no es un idioma\r\nLANG:en\r\nEND:VCARD";
    assert_eq!(
        contact_from(card, "").unwrap().languages,
        vec!["es-AR", "en"]
    );
}

/// Lo que llega del servidor tiene que poder cruzar a la ventana: los nombres
/// de los campos son los que la ventana espera.
#[test]
fn el_contacto_cruza_a_la_ventana_con_los_nombres_esperados() {
    let c = contact_from(&nextcloud(), "https://x/ana.vcf").unwrap();
    let json = serde_json::to_value(&c).unwrap();
    for field in [
        "uid",
        "name",
        "sort_key",
        "emails",
        "phones",
        "organization",
        "notes",
        "url",
        "photo",
        "photo_url",
        "photo_skipped",
        "nickname",
        "title",
        "role",
        "kind",
        "addresses",
        "birthday",
        "anniversary",
        "other_dates",
        "websites",
        "social",
        "languages",
        "time_zone",
        "geo",
        "categories",
        "custom_fields",
    ] {
        assert!(json.get(field).is_some(), "falta {field}");
    }
    assert_eq!(json["birthday"]["year"], serde_json::Value::Null);
    assert_eq!(json["birthday"]["month"], 4);
    assert_eq!(json["addresses"][0]["postal_code"], "B1636");

    let tapada = contact_from(
        "BEGIN:VCARD\r\nFN:Ana\r\nPHOTO;VALUE=uri:http://x/a.jpg\r\nEND:VCARD",
        "",
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&tapada).unwrap()["photo_skipped"],
        "insecure"
    );
}
