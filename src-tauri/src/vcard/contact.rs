//! El contacto que se muestra, y cómo se arma de una tarjeta.
//!
//! ── Lo que se lee ───────────────────────────────────────────────────────────
//!
//! Además del nombre, los correos, los teléfonos, la organización y las notas:
//! la foto (`PHOTO`), las direcciones (`ADR`), el cumpleaños y el aniversario
//! (`BDAY`, `ANNIVERSARY`, y las fechas con etiqueta de Apple), las webs
//! (`URL`), las redes y la mensajería (`IMPP`, `X-SOCIALPROFILE` y los `X-` de
//! cada servicio), el cargo (`TITLE`, `ROLE`), el apodo (`NICKNAME`), los
//! idiomas (`LANG`), la zona horaria (`TZ`), la ubicación (`GEO`), las
//! categorías (`CATEGORIES`), qué es la tarjeta (`KIND`), y los `X-` que no son
//! de nadie, que son los «campos a medida».
//!
//! ── Lo que **no** se lee acá, a propósito ───────────────────────────────────
//!
//! Las relaciones —`RELATED`, `X-ABRELATEDNAMES` de Apple, `X-MANAGERSNAME` y
//! compañía— y los miembros de un grupo (`MEMBER`) son de Vasak-OS/vasak-contacts#6:
//! hay que resolverlas contra los demás contactos, y mostrarlas sueltas como
//! campos a medida sería mostrarlas mal. Tampoco aparecen entre los `X-`.
//!
//! **Y nada de esto escribe**: escribir en la libreta es el #2, y se hace
//! guardando el crudo y tocando sólo lo que cambió (decisión 3 del taller).
//!
//! ── Los grupos de Apple ─────────────────────────────────────────────────────
//!
//! Apple no usa `TYPE` para las etiquetas que no son las de siempre: agrupa la
//! propiedad con una `X-ABLabel` del mismo grupo.
//!
//! ```text
//! item1.URL:https://ana.ejemplo.com
//! item1.X-ABLabel:_$!<HomePage>!$_
//! item2.X-ABDATE:2010-06-12
//! item2.X-ABLabel:_$!<Anniversary>!$_
//! ```
//!
//! Así que las propiedades se leen en dos pasadas: la primera junta las
//! etiquetas por grupo, la segunda arma el contacto.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::date::{date_from, LabeledDate, PartialDate};
use super::lines::{
    decoded, display_value, label_of, split_fields, split_list, split_property, unfold_lines,
    Property,
};
use super::photo::{photo_from, PhotoSkipped, PhotoSource};
use super::social::{is_social, social_from, SocialProfile};

/// Cuántas propiedades se leen de una tarjeta.
///
/// Un contacto real tiene decenas. Mil es un archivo armado para hacer trabajar
/// al programa. La foto no cuenta de más: plegada en cientos de renglones, se
/// vuelve a juntar en una sola propiedad antes de contar.
pub const MAX_PROPERTIES: usize = 1000;

/// Tope de un valor que se muestra.
///
/// Un nombre de diez mil caracteres no es un nombre: es algo que va a romper la
/// lista al dibujarla. La foto tiene su tope propio, más alto: ver
/// [`super::photo::MAX_PHOTO_BYTES`].
pub const MAX_VALUE: usize = 4096;

/// Cuántos elementos se guardan de cada lista —categorías, campos a medida,
/// idiomas—. Una tarjeta con quinientas categorías no es una tarjeta que se
/// pueda leer, y la ficha tiene que seguir siendo una ficha.
pub const MAX_LIST_ITEMS: usize = 50;

/// Una dirección de correo, un teléfono, o cualquier cosa con una etiqueta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabeledValue {
    /// «casa», «trabajo», «celular»… tal como lo escribió quien hizo la
    /// tarjeta, en minúsculas. Vacío si no dijo nada.
    pub label: String,
    pub value: String,
}

/// Una dirección postal, con sus siete partes.
///
/// Se guardan separadas y no ya armadas en renglones: cómo se escribe una
/// dirección depende del país, y lo decide la ventana.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub label: String,
    pub po_box: String,
    pub extended: String,
    pub street: String,
    pub locality: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
}

impl Address {
    fn is_empty(&self) -> bool {
        [
            &self.po_box,
            &self.extended,
            &self.street,
            &self.locality,
            &self.region,
            &self.postal_code,
            &self.country,
        ]
        .iter()
        .all(|p| p.trim().is_empty())
    }
}

/// Un `X-` que no es de nadie: lo que se llama «campo a medida».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomField {
    /// El nombre de la propiedad tal como vino: `X-PHONETIC-FIRST-NAME`.
    pub name: String,
    /// La etiqueta de Apple, si la tiene: es lo que la persona escribió.
    #[serde(default)]
    pub label: String,
    pub value: String,
}

/// Un contacto, listo para mostrar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    /// El identificador de la tarjeta dentro de la libreta.
    pub uid: String,
    /// Cómo se llama, para mostrar.
    pub name: String,
    /// Para ordenar: «Pérez, Ana» en vez de «Ana Pérez».
    ///
    /// Va aparte porque ordenar por el nombre que se muestra pone a todas las
    /// Anas juntas y a los Pérez desparramados, que no es como nadie busca a
    /// alguien en una agenda.
    pub sort_key: String,
    pub emails: Vec<LabeledValue>,
    pub phones: Vec<LabeledValue>,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub notes: String,
    /// La dirección de la tarjeta en el servidor, para volver a buscarla.
    #[serde(default)]
    pub url: String,

    /// La foto como `data:` URI, lista para mostrar. Vacía si no hay.
    #[serde(default)]
    pub photo: String,
    /// La dirección `https` de una foto que está afuera de la tarjeta. **La
    /// ventana no la pide nunca**: la baja el programa y la guarda, ver
    /// `photos.rs`. Vacía si la foto vino adentro o no hay.
    #[serde(default)]
    pub photo_url: String,
    /// Por qué hay una foto que no se muestra, si la hay.
    #[serde(default)]
    pub photo_skipped: Option<PhotoSkipped>,

    #[serde(default)]
    pub nickname: String,
    /// El cargo (`TITLE`): «Jefa de soporte».
    #[serde(default)]
    pub title: String,
    /// La función (`ROLE`): «Programadora». No es lo mismo que el cargo.
    #[serde(default)]
    pub role: String,
    /// `individual`, `group`, `org` o `location`. Vacío si la tarjeta no dice,
    /// que es casi siempre y quiere decir `individual`.
    #[serde(default)]
    pub kind: String,

    #[serde(default)]
    pub addresses: Vec<Address>,
    #[serde(default)]
    pub birthday: Option<PartialDate>,
    #[serde(default)]
    pub anniversary: Option<PartialDate>,
    /// Las otras fechas con etiqueta, como las que guarda Apple.
    #[serde(default)]
    pub other_dates: Vec<LabeledDate>,
    #[serde(default)]
    pub websites: Vec<LabeledValue>,
    #[serde(default)]
    pub social: Vec<SocialProfile>,

    /// Las etiquetas de idioma (`es-AR`): el nombre en el idioma de la sesión
    /// lo pone la ventana.
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub time_zone: String,
    /// «latitud, longitud», si la tarjeta trae una ubicación que se entiende.
    #[serde(default)]
    pub geo: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
}

/// Los `X-` que no se muestran como campos a medida.
///
/// O porque ya se leen como otra cosa, o porque son de las relaciones (#6), o
/// porque son internos de quien exportó la tarjeta: un identificador, una
/// preferencia de cómo mostrarla, la huella de una foto. Mostrar
/// «X-ABUID: 5D4A…» en la ficha no le sirve a nadie.
const HIDDEN_X_PROPERTIES: &[&str] = &[
    // Apple.
    "X-ABLABEL",
    "X-ABUID",
    "X-ABSHOWAS",
    "X-ABADR",
    "X-ABDATE",
    "X-ABRELATEDNAMES",
    "X-ABPERSONFLAGS",
    "X-APPLE-OMIT-YEAR",
    "X-IMAGETYPE",
    "X-IMAGEHASH",
    "X-SHARED-PHOTO-DISPLAY-PREF",
    "X-ADDRESSBOOKSERVER-KIND",
    "X-ADDRESSBOOKSERVER-MEMBER",
    // Las relaciones de Google, Microsoft y Evolution: son del #6.
    "X-MANAGERSNAME",
    "X-ASSISTANTNAME",
    "X-SPOUSE",
    "X-EVOLUTION-SPOUSE",
    "X-EVOLUTION-MANAGER",
    "X-EVOLUTION-ASSISTANT",
    // Fechas que se leen como aniversario.
    "X-ANNIVERSARY",
    "X-EVOLUTION-ANNIVERSARY",
    // Preferencias y marcas internas.
    "X-MOZILLA-HTML",
    "X-MOZILLA-PROPERTY",
    "X-EVOLUTION-FILE-AS",
    "X-EVOLUTION-LIST",
    "X-EVOLUTION-LIST-SHOW-ADDRESSES",
    "X-EVOLUTION-BLOG-URL",
    "X-EVOLUTION-VIDEO-URL",
    "X-KADDRESSBOOK-X-IMADDRESS",
    "X-MS-OL-DEFAULT-POSTAL-ADDRESS",
    "X-MS-CARDPICTURE",
    "X-WAB-GENDER",
];

/// Las categorías que Google agrega solas a todos los contactos: son sus
/// grupos del sistema, no algo que la persona haya puesto.
const GOOGLE_SYSTEM_GROUPS: &[&str] = &["myContacts", "starred"];

/// Deja la etiqueta de Apple en algo que se pueda mostrar.
///
/// Las de siempre vienen envueltas —`_$!<HomePage>!$_`— y se dejan en
/// minúsculas, como las de `TYPE`, para que la ventana las traduzca. Las que
/// escribió la persona («Casa de la playa») vienen tal cual.
fn apple_label(raw: &str) -> String {
    let raw = raw.trim();
    match raw
        .strip_prefix("_$!<")
        .and_then(|r| r.strip_suffix(">!$_"))
    {
        Some(stock) => stock.to_ascii_lowercase(),
        None => raw.to_lowercase(),
    }
}

/// Lee un contacto de una tarjeta.
///
/// `None` si no hay nada que mostrar: una tarjeta sin nombre y sin datos ocupa
/// lugar en la lista y no sirve para nada.
pub fn contact_from(raw: &str, url: &str) -> Option<Contact> {
    let properties: Vec<Property> = unfold_lines(raw)
        .into_iter()
        .take(MAX_PROPERTIES)
        .filter_map(|line| split_property(&line))
        .collect();

    // Primera pasada: las etiquetas de Apple, por grupo.
    let apple_labels: HashMap<&str, String> = properties
        .iter()
        .filter(|p| !p.group.is_empty() && p.name == "X-ABLABEL")
        .map(|p| (p.group.as_str(), clipped(&apple_label(&display_value(p)))))
        .collect();

    // Segunda pasada: el contacto.
    let mut builder = ContactBuilder {
        contact: Contact {
            url: url.to_string(),
            ..Default::default()
        },
        apple_labels,
        structured_name: Vec::new(),
        photo_seen: false,
        kind_from_apple: String::new(),
    };
    for property in &properties {
        builder.add(property);
    }
    builder.finish()
}

/// Lo que se va juntando mientras se leen las propiedades de una tarjeta.
///
/// Una función por propiedad con trabajo propio, en vez de un solo `match` con
/// todo adentro: así cada regla se lee sola y se prueba sola.
struct ContactBuilder<'a> {
    contact: Contact,
    apple_labels: HashMap<&'a str, String>,
    structured_name: Vec<String>,
    photo_seen: bool,
    kind_from_apple: String,
}

impl ContactBuilder<'_> {
    /// La etiqueta de una propiedad: la de Apple si la tiene, que es lo que
    /// escribió la persona, y si no la del `TYPE`.
    fn label_for(&self, property: &Property) -> String {
        self.apple_labels
            .get(property.group.as_str())
            .filter(|l| !l.is_empty())
            .cloned()
            .unwrap_or_else(|| clipped(&label_of(&property.params)))
    }

    fn add(&mut self, property: &Property) {
        let c = &mut self.contact;
        match property.name.as_str() {
            "UID" => c.uid = clipped(&display_value(property)),
            "FN" => c.name = clipped(&display_value(property)),
            "N" => self.structured_name = split_fields(&decoded(property)),
            // En la 4.0 la dirección viene como `mailto:ana@x`. Dejarlo haría que
            // el botón de escribirle abriera «mailto:mailto:…».
            "EMAIL" => self.add_with_scheme(property, "mailto:", |c| &mut c.emails),
            "TEL" => self.add_with_scheme(property, "tel:", |c| &mut c.phones),
            "ORG" => c.organization = organization_from(property),
            "NOTE" => c.notes = clipped(&display_value(property)),
            "PHOTO" => self.add_photo(property),
            "ADR" => self.add_address(property),
            "BDAY" if c.birthday.is_none() => c.birthday = date_from(property, clipped),
            "ANNIVERSARY" | "X-ANNIVERSARY" | "X-EVOLUTION-ANNIVERSARY"
                if c.anniversary.is_none() =>
            {
                c.anniversary = date_from(property, clipped)
            }
            "X-ABDATE" => self.add_labeled_date(property),
            "URL" if c.websites.len() < MAX_LIST_ITEMS => {
                let value = display_value(property).trim().to_string();
                let label = self.label_for(property);
                push_labeled(&mut self.contact.websites, label, value);
            }
            "TITLE" => c.title = clipped(display_value(property).trim()),
            "ROLE" => c.role = clipped(display_value(property).trim()),
            "NICKNAME" => c.nickname = clipped(&split_list(&decoded(property)).join(", ")),
            "KIND" | "X-ADDRESSBOOKSERVER-KIND" => {
                if let Some(kind) = kind_from(property) {
                    c.kind = kind;
                }
            }
            // Apple dice «es una empresa» así.
            "X-ABSHOWAS"
                if display_value(property)
                    .trim()
                    .eq_ignore_ascii_case("company") =>
            {
                self.kind_from_apple = "org".into();
            }
            "LANG" => self.add_language(property),
            "TZ" if c.time_zone.is_empty() => c.time_zone = clipped(display_value(property).trim()),
            "GEO" if c.geo.is_empty() => {
                c.geo = geo_from(&display_value(property)).unwrap_or_default()
            }
            "CATEGORIES" => self.add_categories(property),
            name if is_social(name) => self.add_social(property),
            name if name.starts_with("X-") && !HIDDEN_X_PROPERTIES.contains(&name) => {
                self.add_custom_field(property)
            }
            _ => {}
        }
    }

    fn add_with_scheme(
        &mut self,
        property: &Property,
        scheme: &str,
        target: fn(&mut Contact) -> &mut Vec<LabeledValue>,
    ) {
        let value = display_value(property);
        let value = strip_scheme(&value, scheme).trim().to_string();
        let label = self.label_for(property);
        push_labeled(target(&mut self.contact), label, value);
    }

    /// **Sólo la primera foto que se pueda usar**, y se decodifica una vez: una
    /// tarjeta con veinte fotos no hace veinte veces el trabajo.
    fn add_photo(&mut self, property: &Property) {
        if self.photo_seen {
            return;
        }
        let c = &mut self.contact;
        match photo_from(property) {
            Some(PhotoSource::Inline(uri)) => {
                c.photo = uri;
                c.photo_url.clear();
                c.photo_skipped = None;
                self.photo_seen = true;
            }
            Some(PhotoSource::Remote(remote)) => {
                c.photo_url = remote;
                c.photo_skipped = None;
                self.photo_seen = true;
            }
            // Una que no sirve no tapa a la siguiente: se anota por qué, y se
            // sigue buscando.
            Some(PhotoSource::Skipped(why)) if c.photo_skipped.is_none() => {
                c.photo_skipped = Some(why);
            }
            Some(PhotoSource::Skipped(_)) | None => {}
        }
    }

    /// Se decodifica antes de partir y se desescapa después, como el `N`: al
    /// revés, un punto y coma escapado partía la calle en dos.
    fn add_address(&mut self, property: &Property) {
        if self.contact.addresses.len() >= MAX_LIST_ITEMS {
            return;
        }
        let fields = split_fields(&decoded(property));
        let field = |i: usize| clipped(fields.get(i).map(|f| f.trim()).unwrap_or(""));
        let address = Address {
            label: self.label_for(property),
            po_box: field(0),
            extended: field(1),
            street: field(2),
            locality: field(3),
            region: field(4),
            postal_code: field(5),
            country: field(6),
        };
        if !address.is_empty() {
            self.contact.addresses.push(address);
        }
    }

    /// Las fechas de Apple: la que tiene la etiqueta de aniversario es el
    /// aniversario, y el resto va con la etiqueta que tenga.
    fn add_labeled_date(&mut self, property: &Property) {
        let Some(date) = date_from(property, clipped) else {
            return;
        };
        let label = self.label_for(property);
        let c = &mut self.contact;
        if label == "anniversary" && c.anniversary.is_none() {
            c.anniversary = Some(date);
        } else if c.other_dates.len() < MAX_LIST_ITEMS {
            c.other_dates.push(LabeledDate { label, date });
        }
    }

    /// Una etiqueta de idioma es corta y sin espacios: `es-AR`. Lo que no lo
    /// parece no se guarda, y no se repite.
    fn add_language(&mut self, property: &Property) {
        let tag = display_value(property).trim().to_string();
        let looks_like_tag = !tag.is_empty()
            && tag.len() <= 35
            && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let languages = &mut self.contact.languages;
        if looks_like_tag && languages.len() < MAX_LIST_ITEMS && !languages.contains(&tag) {
            languages.push(tag);
        }
    }

    fn add_categories(&mut self, property: &Property) {
        let categories = &mut self.contact.categories;
        let mut seen: HashSet<String> = categories.iter().cloned().collect();
        for category in split_list(&decoded(property)) {
            let category = clipped(category.trim());
            let system = GOOGLE_SYSTEM_GROUPS.contains(&category.as_str());
            if !system && categories.len() < MAX_LIST_ITEMS && seen.insert(category.clone()) {
                categories.push(category);
            }
        }
    }

    fn add_social(&mut self, property: &Property) {
        if self.contact.social.len() >= MAX_LIST_ITEMS {
            return;
        }
        let Some(profile) = social_from(property, self.label_for(property)) else {
            return;
        };
        self.contact.social.push(SocialProfile {
            service: clipped(&profile.service),
            label: profile.label,
            handle: clipped(&profile.handle),
            // Una dirección recortada ya no es la dirección: mejor ninguna.
            url: if profile.url.len() <= MAX_VALUE {
                profile.url
            } else {
                String::new()
            },
        });
    }

    fn add_custom_field(&mut self, property: &Property) {
        let value = display_value(property).trim().to_string();
        if value.is_empty() || self.contact.custom_fields.len() >= MAX_LIST_ITEMS {
            return;
        }
        let label = self
            .apple_labels
            .get(property.group.as_str())
            .cloned()
            .unwrap_or_default();
        self.contact.custom_fields.push(CustomField {
            name: clipped(&property.name),
            label,
            value: clipped(&value),
        });
    }

    fn finish(self) -> Option<Contact> {
        let mut contact = self.contact;
        if contact.kind.is_empty() {
            contact.kind = self.kind_from_apple;
        }

        // El nombre que se muestra: el `FN` si está, y si no se arma con el `N`.
        // Una tarjeta sin `FN` es inválida según el estándar y aparece igual,
        // así que armarlo es la diferencia entre ver a alguien y ver un renglón
        // vacío.
        if contact.name.trim().is_empty() {
            contact.name = display_name(&self.structured_name);
        }
        contact.sort_key = sort_name(&self.structured_name, &contact.name);

        let has_something = !contact.name.trim().is_empty()
            || !contact.emails.is_empty()
            || !contact.phones.is_empty();
        has_something.then_some(contact)
    }
}

/// `ORG` trae la empresa y sus divisiones separadas por punto y coma. Se
/// muestran juntas y no sólo la primera: «Vasak Group» y «Vasak Group,
/// Soporte» son cosas distintas.
fn organization_from(property: &Property) -> String {
    clipped(
        &split_fields(&decoded(property))
            .into_iter()
            .filter(|c| !c.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// `individual`, `group`, `org` o `location`; lo que no es ninguno no se
/// guarda.
fn kind_from(property: &Property) -> Option<String> {
    let kind = display_value(property).trim().to_ascii_lowercase();
    matches!(kind.as_str(), "individual" | "group" | "org" | "location").then_some(kind)
}

/// «latitud, longitud» de un `GEO`, de la 3.0 (`37.38;-122.08`) o de la 4.0
/// (`geo:37.38,-122.08`). `None` si no son dos números en rango.
fn geo_from(value: &str) -> Option<String> {
    let value = value.trim();
    let value = strip_scheme(value, "geo:");
    // En la 4.0 puede venir una incertidumbre detrás: `geo:37.38,-122.08;u=35`.
    let coordinates = value.split(';').collect::<Vec<_>>();
    let (lat, lon) = if value.contains(',') {
        let first = coordinates.first()?;
        let mut parts = first.split(',');
        (parts.next()?, parts.next()?)
    } else {
        (*coordinates.first()?, *coordinates.get(1)?)
    };
    let lat: f64 = lat.trim().parse().ok()?;
    let lon: f64 = lon.trim().parse().ok()?;
    let in_range = lat.is_finite()
        && lon.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon);
    in_range.then(|| format!("{lat}, {lon}"))
}

/// Saca el esquema de un valor, **sin mirar mayúsculas**.
///
/// Los esquemas de una URI no las distinguen: `MailTo:` y `TEL:` son tan
/// válidos como los de minúscula, y los escriben los exportadores de verdad.
/// Dejarlos pegados hace que el botón de escribir abra «mailto:MailTo:…» y que
/// el de llamar reciba algo que no es un número.
///
/// El `is_char_boundary` **no es opcional**: `&value[..scheme.len()]` corta en
/// un índice de byte, y con un valor que tenga un carácter multibyte antes del
/// esquema —un contacto con el nombre `日本:mailto:…`, o un teléfono con un
/// emoji— ese corte cae en medio de un carácter y **entra en pánico**. Un
/// `mailto:` de otro mundo, de un solo byte, hace que reviente.
fn strip_scheme<'a>(value: &'a str, scheme: &str) -> &'a str {
    if value.len() >= scheme.len()
        && value.is_char_boundary(scheme.len())
        && value[..scheme.len()].eq_ignore_ascii_case(scheme)
    {
        return &value[scheme.len()..];
    }
    value
}

fn push_labeled(target: &mut Vec<LabeledValue>, label: String, value: String) {
    if value.is_empty() {
        return;
    }
    target.push(LabeledValue {
        label,
        value: clipped(&value),
    });
}

/// Un valor que no rompa la lista al dibujarla.
pub(super) fn clipped(value: &str) -> String {
    if value.len() <= MAX_VALUE {
        return value.to_string();
    }
    let mut cut = MAX_VALUE;
    while cut > 0 && !value.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &value[..cut])
}

/// Arma «Ana Pérez» a partir del `N`, que viene al revés y por partes.
///
/// El orden del campo es apellido, nombre, segundos nombres, tratamiento y
/// sufijo. Mostrarlo tal cual daría «Pérez;Ana;;Sra.;».
fn display_name(fields: &[String]) -> String {
    let field = |i: usize| fields.get(i).map(String::as_str).unwrap_or("").trim();
    [field(3), field(1), field(2), field(0), field(4)]
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

/// «Pérez, Ana»: cómo se busca a alguien en una agenda.
///
/// Ordenar por el nombre que se muestra pone a todas las Anas juntas y a los
/// Pérez desparramados, que no es como nadie busca.
fn sort_name(fields: &[String], shown: &str) -> String {
    let field = |i: usize| fields.get(i).map(String::as_str).unwrap_or("").trim();
    let surname = field(0);
    let given = field(1);

    match (surname.is_empty(), given.is_empty()) {
        (false, false) => format!("{surname}, {given}"),
        (false, true) => surname.to_string(),
        // Sin apellido, se ordena por lo que se muestra: es lo único que hay.
        _ => shown.to_string(),
    }
}
