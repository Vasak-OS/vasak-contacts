//! Los topes de lo que llega de un servidor CardDAV, y la forma de no mandarle
//! la credencial a otro.
//!
//! Viene de `sync/src/dav/webdav.rs` de `vasak-accounts`, con sus pruebas. Lo
//! que se trae es lo genérico del DAV —los topes de forma del XML y la
//! resolución de direcciones—, que es lo mismo para CardDAV que para CalDAV.
//! Lo que es de este protocolo —los espacios de nombres, las consultas, el
//! `multistatus`— se queda en `carddav.rs`.
//!
//! ── Qué se le cree al servidor, y qué no ────────────────────────────────────
//!
//! El servidor es el de la persona, pero lo que contesta **se lee como si lo
//! hubiera escrito cualquiera**: una respuesta rota, una desmedida o una armada
//! para hacer trabajar al programa no pueden tumbar la aplicación ni llevarse
//! la credencial a otra parte. Por eso:
//!
//! - **Cada respuesta tiene tope** ([`Limits::max_body_bytes`]), cortado mientras
//!   llega y no después de leerla entera.
//! - **El XML sin DTD**: `roxmltree` la rechaza por omisión, que es lo que
//!   cierra las entidades externas y las expansiones en cadena. Y con tope de
//!   nodos, de profundidad, de espacios de nombres y de atributos por elemento,
//!   mirados antes de armarlo ([`parse_xml`]): `roxmltree` baja de forma
//!   recursiva —un anidado de más **aborta el proceso**—, tarda como el cubo con
//!   miles de espacios de nombres y como el cuadrado con miles de atributos en
//!   un elemento.
//! - **Cada dirección que manda el servidor se resuelve contra la colección y
//!   se rechaza si es de otro origen** —esquema, máquina y puerto— antes de
//!   pedirla o guardarla ([`resolve_href`]). La credencial viaja sólo al origen
//!   de la cuenta.
//! - **Sólo `https`**, y sin redirecciones: el pedido lleva la contraseña y una
//!   redirección la mandaría adonde el servidor diga. Ver [`client`].

/// `reqwest::Url` es `url::Url`: la aplicación no depende de `url` directo, y
/// agregar la dependencia para nombrar el mismo tipo no suma nada.
use reqwest::Url;

/// El largo máximo de una dirección que manda el servidor. Una de más es algo
/// armado para llenar la base, no una libreta.
pub const MAX_HREF_BYTES: usize = 2048;

/// Hasta cuánto del detalle de un error se guarda para el diario.
pub const MAX_ERROR_DETAIL_BYTES: usize = 200;

/// Los topes de lo que se lee de la red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Lo que se lee de una respuesta. Una agenda de años puede ser grande,
    /// pero no ilimitado: sin tope, un servidor que devuelve basura hace crecer
    /// la memoria de la ventana sin freno. Ocho megas son de sobra para una
    /// respuesta de CalDAV y cortan antes de que la memoria crezca a donde
    /// quiera.
    pub max_body_bytes: usize,
    /// Nodos de un documento XML. Un `REPORT` de veinte mil eventos son unos
    /// ciento sesenta mil; un millón es un documento hecho de etiquetas vacías.
    pub max_xml_nodes: u32,
    /// Niveles de anidado de un documento XML. Un `multistatus` de verdad tiene
    /// menos de diez. `roxmltree` baja por los elementos de forma recursiva y
    /// sin tope propio: 200 000 niveles dan *stack overflow*, y eso **aborta el
    /// proceso** entero, sin pánico que atrapar.
    pub max_xml_depth: usize,
    /// Espacios de nombres **distintos** —cada par prefijo y dirección— que
    /// declara un documento. Un servidor de verdad usa entre tres y seis. Con
    /// miles, `roxmltree` tarda un tiempo que crece como el cubo: copia los que
    /// están a la vista en cada elemento que declara uno.
    ///
    /// Distintos y no cada declaración: hay servidores que repiten
    /// `xmlns="DAV:"` en cada elemento, y eso no le cuesta nada a `roxmltree`.
    /// Lo que cuesta es cuántos hay a la vista a la vez.
    pub max_xml_namespaces: usize,
    /// Atributos de un mismo elemento, con sus declaraciones `xmlns`. Un
    /// elemento DAV de verdad lleva de cero a tres, más sus `xmlns`: menos de
    /// diez. `roxmltree` busca el repetido comparando cada atributo con todos
    /// los anteriores del elemento, así que tarda como el cuadrado: sesenta mil
    /// atributos vacíos son siete segundos en release.
    pub max_xml_attributes: usize,
}

impl Limits {
    /// Lo mismo que [`Limits`], con los números que se eligieron medidos sobre
    /// `vasak-accounts`, donde los usa la misma biblioteca de DAV para las mismas
    /// respuestas. Que sea el mismo número en los dos lados es lo que hace
    /// comparables las dos auditías.
    pub const DEFAULT: Limits = Limits {
        max_body_bytes: 16 * 1024 * 1024,
        max_xml_nodes: 1_000_000,
        max_xml_depth: 64,
        max_xml_namespaces: 32,
        max_xml_attributes: 64,
    };
}

/// Lo que puede salir mal hablando con un servidor DAV.
///
/// El texto de cada uno (`Display`) **es fijo**: sin direcciones y sin nada que
/// haya escrito el servidor, ni siquiera de paso. Sale tal cual en el estado que
/// muestra la ventana, que lo lee cualquiera de la sesión.
///
/// Los dos que llevan algo del servidor —la red y el XML— lo llevan en
/// [`DavError::network`] y [`DavError::bad_xml`], recortado a
/// [`MAX_ERROR_DETAIL_BYTES`] y **nunca en el texto que ve la persona**: el
/// error de `roxmltree` lleva nombres de etiquetas y prefijos del documento, y
/// el de la red los del certificado del otro lado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DavError {
    /// La dirección de la cuenta no es `https`, o trae usuario y contraseña.
    InsecureUrl,
    /// Una dirección de otro origen que el de la cuenta.
    ForeignOrigin,
    /// 401: la credencial no vale.
    Unauthorized,
    /// El servidor quiso redirigir. No se sigue: el pedido lleva la credencial.
    Redirect(u16),
    /// Cualquier otro estado que no se esperaba.
    Status(u16),
    /// La respuesta pasó el tope.
    BodyTooLarge(usize),
    /// No se pudo hablar con el servidor. El detalle, recortado, es para el
    /// diario.
    Network(String),
    /// La respuesta no es un XML que se entienda, o no se puede mirar. El
    /// detalle, recortado, es para el diario.
    BadXml(String),
}

impl std::fmt::Display for DavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DavError::InsecureUrl => f.write_str(
                "la dirección de la cuenta no está cifrada: por ahí la contraseña viajaría a la \
                 vista de cualquiera en la red",
            ),
            DavError::ForeignOrigin => {
                f.write_str("el servidor mandó una dirección de otro servidor, y no se sigue")
            }
            DavError::Unauthorized => f.write_str(
                "el servidor rechazó el usuario o la contraseña; volvé a conectar la cuenta \
                 desde Configuración",
            ),
            DavError::Redirect(status) => write!(
                f,
                "el servidor respondió {status} para mandar a otra dirección, y no se sigue: el \
                 pedido lleva la credencial"
            ),
            DavError::Status(status) => write!(f, "el servidor respondió {status}"),
            DavError::BodyTooLarge(cap) => write!(
                f,
                "el servidor mandó más de {cap} bytes, que es lo que se lee de una vez"
            ),
            DavError::Network(_) => f.write_str("no se pudo hablar con el servidor"),
            DavError::BadXml(_) => f.write_str("el servidor contestó algo que no se entiende"),
        }
    }
}

impl std::error::Error for DavError {}

/// Lo primero de un texto, hasta `cap` bytes y sin partir un carácter.
fn clipped(text: &str, cap: usize) -> String {
    if text.len() <= cap {
        return text.to_string();
    }
    let mut cut = cap;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &text[..cut])
}

impl DavError {
    /// Un XML que no se entiende, con el detalle recortado para el diario.
    pub fn bad_xml(detail: impl std::fmt::Display) -> Self {
        DavError::BadXml(clipped(&detail.to_string(), MAX_ERROR_DETAIL_BYTES))
    }

    /// No se pudo hablar con el servidor, con el detalle recortado para el
    /// diario.
    pub fn network(detail: impl std::fmt::Display) -> Self {
        DavError::Network(clipped(&detail.to_string(), MAX_ERROR_DETAIL_BYTES))
    }
}

// ---------------------------------------------------------------------------
// El cliente
// ---------------------------------------------------------------------------

/// El cliente HTTP de CalDAV, con las dos cosas que no se negocian.
///
/// **Sólo `https`** —la cuenta está por entregar una contraseña o un token— y
/// **sin redirecciones**, porque el pedido lleva la credencial y una
/// redirección la mandaría adonde el servidor diga. Las dos se ponen acá y no en
/// cada llamada, para que no haya un camino que se las saltee.
///
/// Devuelve un `String` y no un [`DavError`] porque **esto se ve en la ventana**:
/// el estado que se muestra es este texto, y tiene que poder decir qué hacer
/// («volvé a conectar la cuenta desde Configuración»), no qué exception salió.
pub fn client(timeout: std::time::Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(timeout)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("VasakOS")
        .build()
        .map_err(|e| {
            let detalle = DavError::network(e.without_url());
            format!("no se pudo preparar la conexión con el servidor: {detalle}")
        })
}

// ---------------------------------------------------------------------------
// Las direcciones
// ---------------------------------------------------------------------------

/// La dirección que guardó el servicio al conectar, si se puede usar.
///
/// Es la primera mitad de la defensa: si la cuenta no tiene una dirección
/// `https` y sin usuario y contraseña adentro, no se le manda nada. La segunda
/// mitad es [`resolve_href`], que es la que revisa lo que manda el servidor.
///
/// Separa las dos porque los dos textos se leen en lugares distintos: éste
/// aparece cuando la persona intenta abrir un calendario y es un problema de la
/// cuenta; el otro aparece cuando el servidor dijo algo raro y no tiene nada
/// que ver con la cuenta.
pub fn parse_account_url(guardada: &str) -> Result<Url, DavError> {
    let trimmed = guardada.trim();
    let url = Url::parse(trimmed).map_err(|_| DavError::InsecureUrl)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return Err(DavError::InsecureUrl);
    }
    Ok(url)
}

/// Resuelve una dirección que mandó el servidor contra la colección en la que
/// vino, y la rechaza si es de otro origen.
///
/// Los servidores contestan con una ruta absoluta casi siempre, con una URL
/// entera a veces y con una relativa de vez en cuando: [`Url::join`]
/// resuelve las tres, y pegarlas a mano rompería la segunda.
///
/// **Otro origen —esquema, máquina o puerto— es `ForeignOrigin`**: pedirla
/// mandaría la credencial de la cuenta a quien diga el servidor, y guardarla
/// haría que la próxima vuelta la pida. También se rechaza una con usuario y
/// contraseña adentro, o una desmedida, antes o después de resolverla. El
/// fragmento se descarta: no llega al servidor, así que dos direcciones que
/// sólo difieren en él son la misma.
pub fn resolve_href(base: &Url, href: &str) -> Result<Url, DavError> {
    let href = href.trim();
    if href.is_empty() || href.len() > MAX_HREF_BYTES {
        return Err(DavError::ForeignOrigin);
    }
    let mut resolved = base.join(href).map_err(|_| DavError::ForeignOrigin)?;
    // Otra vez después de resolver: `join` codifica, y dos mil espacios son
    // seis mil bytes de `%20`.
    if resolved.as_str().len() > MAX_HREF_BYTES
        || resolved.origin() != base.origin()
        || !resolved.username().is_empty()
        || resolved.password().is_some()
    {
        return Err(DavError::ForeignOrigin);
    }
    resolved.set_fragment(None);
    Ok(resolved)
}

// ---------------------------------------------------------------------------
// El XML
// ---------------------------------------------------------------------------

/// Lee un documento, **sin DTD**, con tope de nodos, de profundidad, de espacios
/// de nombres y de atributos por elemento.
///
/// Un XML que no se entiende es un error y no una lista vacía: una respuesta
/// cortada a la mitad —una conexión que se interrumpió, un servidor que
/// contestó una página de error— no puede verse igual que «esta cuenta no tiene
/// calendarios».
///
/// Antes de `roxmltree` pasa [`check_shape`], una lectura lineal que mira la
/// profundidad, los espacios de nombres y los atributos de cada elemento: son
/// los topes que `roxmltree` no tiene y que un documento chico puede usar para
/// tumbar o trabar la aplicación.
pub fn parse_xml<'a>(xml: &'a str, limits: &Limits) -> Result<roxmltree::Document<'a>, DavError> {
    check_shape(xml, limits).map_err(DavError::bad_xml)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: limits.max_xml_nodes,
        ..roxmltree::ParsingOptions::default()
    };
    roxmltree::Document::parse_with_options(xml, options).map_err(DavError::bad_xml)
}

/// Cuánto trabajo de espacios de nombres se le deja hacer a `roxmltree` en un
/// documento.
///
/// Cada elemento que declara alguno le cuesta, más o menos, el cuadrado de los
/// que hay a la vista. Con el tope de distintos eso es mil por elemento, y un
/// millón de elementos así son dos segundos de CPU en release. Un servidor que
/// repite `xmlns` en cada elemento tiene tres o cuatro a la vista: dieciséis
/// por elemento, y con este tope le alcanza para un millón de elementos, que
/// es el tope de nodos.
const NAMESPACE_WORK_BUDGET: usize = 16_000_000;

/// Lo que dice [`check_shape`] de un elemento con más atributos que
/// [`Limits::max_xml_attributes`].
const TOO_MANY_ATTRIBUTES: &str = "un elemento con demasiados atributos";

/// Busca `needle` desde `from`, y devuelve dónde **termina**.
fn end_of(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|at| from + at + needle.len())
}

/// Mira la forma de un documento sin armarlo: cuántos niveles de anidado tiene,
/// cuántos espacios de nombres distintos declara y cuántos atributos lleva cada
/// elemento.
///
/// Una sola pasada por los bytes, sin recursión y sin guardar nada que crezca
/// con el documento: la memoria es la de los espacios de nombres que ya se
/// vieron, que tienen tope. Salta comentarios, `CDATA` e instrucciones de
/// proceso, y dentro de una etiqueta respeta las comillas de los atributos —un
/// `>` o un `xmlns` dentro de un valor no cuentan— y reconoce la que se cierra
/// sola (`/>`), que no abre un nivel. Una DTD se rechaza acá mismo, que es lo
/// que igual haría `roxmltree`.
///
/// Los atributos se cuentan por su `=` fuera de comillas —todo atributo tiene
/// uno, y un nombre de elemento no puede llevarlo—, así que cuentan también los
/// que vienen pegados sin espacio (`a="1"b="2"`), y las declaraciones `xmlns`
/// cuentan como uno más.
///
/// No valida el XML: eso lo hace `roxmltree` después. Lo que no entiende lo
/// rechaza, y un documento bien formado nunca cae acá por algo que no sea uno
/// de los topes.
pub fn check_shape(xml: &str, limits: &Limits) -> Result<(), &'static str> {
    let bytes = xml.as_bytes();
    let mut depth = 0usize;
    let mut namespaces: Vec<Declaration<'_>> = Vec::new();
    let mut namespace_work = 0usize;
    let mut at = 0;

    while let Some(offset) = bytes[at..].iter().position(|&b| b == b'<') {
        let start = at + offset;
        let rest = &bytes[start..];
        if rest.starts_with(b"<!--") {
            at = end_of(bytes, start + 4, b"-->").ok_or("un comentario sin cerrar")?;
            continue;
        }
        if rest.starts_with(b"<![CDATA[") {
            at = end_of(bytes, start + 9, b"]]>").ok_or("un CDATA sin cerrar")?;
            continue;
        }
        if rest.starts_with(b"<?") {
            at = end_of(bytes, start + 2, b"?>").ok_or("una instrucción sin cerrar")?;
            continue;
        }
        if rest.starts_with(b"<!") {
            return Err("trae una DTD");
        }
        if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            at = end_of(bytes, start + 2, b">").ok_or("una etiqueta sin cerrar")?;
            continue;
        }

        // Una etiqueta que abre: hasta su `>`, fuera de comillas.
        let mut i = start + 1;
        let mut quote: Option<u8> = None;
        let mut after_space = false;
        let mut last = 0u8;
        let mut declares = false;
        let mut attributes = 0usize;
        loop {
            let &c = bytes.get(i).ok_or("una etiqueta sin cerrar")?;
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            match c {
                b'>' => break,
                b'"' | b'\'' => quote = Some(c),
                c if c.is_ascii_whitespace() => {
                    after_space = true;
                    i += 1;
                    continue;
                }
                b'=' => {
                    attributes += 1;
                    if attributes > limits.max_xml_attributes {
                        return Err(TOO_MANY_ATTRIBUTES);
                    }
                }
                _ if after_space => {
                    if let Some((declared, end)) = namespace_at(bytes, i)? {
                        attributes += 1;
                        if attributes > limits.max_xml_attributes {
                            return Err(TOO_MANY_ATTRIBUTES);
                        }
                        declares = true;
                        if !namespaces.contains(&declared) {
                            namespaces.push(declared);
                            if namespaces.len() > limits.max_xml_namespaces {
                                return Err("declara demasiados espacios de nombres");
                            }
                        }
                        last = b'"';
                        after_space = false;
                        i = end;
                        continue;
                    }
                }
                _ => {}
            }
            after_space = false;
            last = c;
            i += 1;
        }

        if declares {
            namespace_work = namespace_work.saturating_add(namespaces.len().pow(2));
            if namespace_work > NAMESPACE_WORK_BUDGET {
                return Err("declara espacios de nombres en demasiados elementos");
            }
        }
        if last != b'/' {
            depth += 1;
            if depth > limits.max_xml_depth {
                return Err("está anidado de más");
            }
        }
        at = i + 1;
    }

    Ok(())
}

/// Una declaración de espacio de nombres: el prefijo (vacío para el de
/// omisión) y la dirección, tal como vinieron.
type Declaration<'a> = (&'a [u8], &'a [u8]);

/// Si en `at` empieza la declaración de un espacio de nombres —`xmlns="…"` o
/// `xmlns:p="…"`—, el par (prefijo, dirección) tal como vino y dónde termina el
/// valor. `xmlnsx="…"` es un atributo cualquiera.
fn namespace_at(bytes: &[u8], at: usize) -> Result<Option<(Declaration<'_>, usize)>, &'static str> {
    let Some(rest) = bytes.get(at..).filter(|r| r.starts_with(b"xmlns")) else {
        return Ok(None);
    };
    let mut i = 5;
    let prefix = match rest.get(i) {
        Some(b':') => {
            let begin = i + 1;
            i = begin;
            while rest
                .get(i)
                .is_some_and(|&c| c != b'=' && !c.is_ascii_whitespace() && c != b'>')
            {
                i += 1;
            }
            &rest[begin..i]
        }
        Some(&c) if c == b'=' || c.is_ascii_whitespace() => &rest[i..i],
        _ => return Ok(None),
    };
    while rest.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if rest.get(i) != Some(&b'=') {
        return Err("un atributo sin valor");
    }
    i += 1;
    while rest.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    let quote = match rest.get(i) {
        Some(&q) if q == b'"' || q == b'\'' => q,
        _ => return Err("un atributo sin comillas"),
    };
    let begin = i + 1;
    let length = rest[begin..]
        .iter()
        .position(|&c| c == quote)
        .ok_or("un atributo sin cerrar")?;
    let value = &rest[begin..begin + length];
    Ok(Some(((prefix, value), at + begin + length + 1)))
}

/// Corre un trabajo de CPU —leer un XML, desarmar tarjetas— fuera de los hilos
/// del bucle de eventos.
///
/// Un documento de ocho megas tarda lo suyo aun con los topes, y mientras ocupa
/// un hilo del bucle, la ventana no atiende nada más. Si el trabajo cae con un
/// pánico, vuelve como un XML que no se entiende y no se lleva la tarea puesta.
pub async fn off_runtime<T, F>(work: F) -> Result<T, DavError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, DavError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| DavError::bad_xml("la lectura de la respuesta se cayó"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Corre un trabajo con un reloj encima.
    ///
    /// El hilo aparte es lo que hace que la prueba se pueda escribir sin fear: si
    /// el trabajo se colgara o desbordara la pila, se pierde el hilo, no el
    /// archivo de pruebas entero. Con el arreglo puesto el trabajo no desborda —
    /// [`check_shape`] rechaza el documento antes de que `roxmltree` lo baje—, y
    /// este es el reloj que avisa si alguna vez volviera a hacerlo.
    fn finishes_within<T: Send + 'static>(
        budget: std::time::Duration,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        let (done, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(work());
        });
        wait.recv_timeout(budget)
            .unwrap_or_else(|_| panic!("no terminó en {budget:?}"))
    }

    fn nested(levels: usize) -> String {
        format!(
            r#"<d:multistatus xmlns:d="DAV:">{}{}</d:multistatus>"#,
            "<d:x>".repeat(levels),
            "</d:x>".repeat(levels)
        )
    }

    /// Un `multistatus` con `declarations` espacios de nombres propios —además
    /// de `d`— y `plain` atributos vacíos, pegados o con espacio.
    fn with_attributes(declarations: usize, plain: usize, separator: &str) -> String {
        let declared: String = (0..declarations)
            .map(|i| format!(r#" xmlns:p{i}="u{i}""#))
            .collect();
        let attributes: String = (0..plain)
            .map(|i| format!(r#"{separator}a{i}="""#))
            .collect();
        format!(r#"<d:multistatus xmlns:d="DAV:"{declared}{attributes}/>"#)
    }

    // ── La fuga de credencial, que es lo grave ──────────────────────────────

    /// **Una dirección de otro origen no se pide.** Es lo que hacía que un
    /// servidor se quedara con la credencial de la cuenta: el `href` del
    /// `multistatus` se resolvía con `Url::join` y nada más, y esa dirección era
    /// la que después llevaba la cabecera `Authorization`.
    #[test]
    fn una_direccion_de_otro_origen_se_rechaza() {
        let base = parse_account_url("https://nube.ejemplo.com/dav/calendars/ana/").unwrap();

        for ajeno in [
            "https://atacante.ejemplo.com/robo/",
            "http://nube.ejemplo.com/dav/",
            // Otra máquina, y otro puerto en la misma: el origen son las tres.
            "https://otra.ejemplo.com/dav/",
            "https://nube.ejemplo.com:8443/dav/",
            "https://usuario@nube.ejemplo.com/dav/",
        ] {
            assert_eq!(
                resolve_href(&base, ajeno),
                Err(DavError::ForeignOrigin),
                "{ajeno} no debería pedirse",
            );
        }
    }

    /// Y del mismo origen sí, en las tres formas que usan los servidores: ruta
    /// absoluta, URL entera y relativa.
    #[test]
    fn una_direccion_del_mismo_origen_se_resuelve() {
        let base = parse_account_url("https://nube.ejemplo.com/dav/calendars/ana/").unwrap();

        for (href, esperado) in [
            (
                "/dav/calendars/ana/personal/",
                "https://nube.ejemplo.com/dav/calendars/ana/personal/",
            ),
            (
                "https://nube.ejemplo.com/dav/calendars/ana/trabajo/",
                "https://nube.ejemplo.com/dav/calendars/ana/trabajo/",
            ),
            (
                "personal/",
                "https://nube.ejemplo.com/dav/calendars/ana/personal/",
            ),
            // El fragmento no llega al servidor: se descarta, y el mismo
            // recurso con y sin él es el mismo.
            (
                "/dav/calendars/ana/personal/#ano",
                "https://nube.ejemplo.com/dav/calendars/ana/personal/",
            ),
        ] {
            let resuelta = resolve_href(&base, href).unwrap_or_else(|e| panic!("{href}: {e}"));
            assert_eq!(resuelta.as_str(), esperado, "{href}");
        }
    }

    /// Lo que no puede ser una dirección: vacía, con la contraseña adentro, o
    /// desmedida. La desmedida se mide **después** de resolver también, porque
    /// `join` codifica: dos mil espacios son seis mil bytes de `%20`.
    #[test]
    fn una_direccion_imposible_se_rechaza() {
        let base = parse_account_url("https://nube.ejemplo.com/dav/calendars/ana/").unwrap();

        assert_eq!(resolve_href(&base, ""), Err(DavError::ForeignOrigin));
        assert_eq!(resolve_href(&base, "   "), Err(DavError::ForeignOrigin));
        assert_eq!(
            resolve_href(&base, "https://ana:secreto@nube.ejemplo.com/dav/"),
            Err(DavError::ForeignOrigin),
        );

        let corto = format!("/dav/{}/", "a".repeat(MAX_HREF_BYTES));
        assert!(
            resolve_href(&base, &corto).is_err(),
            "una de {MAX_HREF_BYTES} bytes"
        );

        // El mismo límite después de codificar.
        let espacios = format!("/dav/{}/", " ".repeat(MAX_HREF_BYTES));
        assert!(resolve_href(&base, &espacios).is_err());
    }

    /// La dirección **guardada** también se mira, y no sólo con un prefijo.
    /// `https://ana:secreto@nube.ejemplo.com/dav/` empieza con `https://` y
    /// tiene la credencial dentro de la URL.
    #[test]
    fn una_direccion_guardada_sin_cifrar_o_con_password_no_se_usa() {
        for mala in [
            "http://nube.ejemplo.com/dav/",
            "ftp://nube.ejemplo.com/dav/",
            "https://ana:secreto@nube.ejemplo.com/dav/",
            "https://ana@nube.ejemplo.com/dav/",
            "no es una dirección",
            "",
            "https://",
        ] {
            assert_eq!(
                parse_account_url(mala),
                Err(DavError::InsecureUrl),
                "{mala} no debería usarse",
            );
        }
        assert!(parse_account_url("https://nube.ejemplo.com/dav/").is_ok());
    }

    // ── Los topes de forma ──────────────────────────────────────────────────

    /// **Un XML anidado de más no tumba la aplicación.** `roxmltree` baja por los
    /// elementos de forma recursiva: cien mil niveles —trescientos kilobytes,
    /// muy por debajo del tope del cuerpo— desbordan la pila, y un desborde de
    /// pila **aborta el proceso entero**, sin pánico que atrapar. Tiene que
    /// volver como un error, y la ventana seguir.
    #[test]
    fn un_xml_demasiado_anidado_se_rechaza_sin_caer() {
        let deep = nested(100_000);
        assert!(deep.len() < 2 * 1024 * 1024);
        assert!(matches!(
            parse_xml(&deep, &Limits::DEFAULT),
            Err(DavError::BadXml(_))
        ));
        // Y el tope es justo: el multistatus más sus niveles.
        let limit = Limits::DEFAULT.max_xml_depth;
        assert!(parse_xml(&nested(limit - 1), &Limits::DEFAULT).is_ok());
        assert!(parse_xml(&nested(limit), &Limits::DEFAULT).is_err());
    }

    /// Lo que no abre un nivel no cuenta: la etiqueta que se cierra sola, un
    /// comentario, un `CDATA`, una instrucción de proceso, y un `>` o un `xmlns`
    /// dentro del valor de un atributo, con comillas dobles o simples.
    #[test]
    fn la_forma_del_xml_respeta_comentarios_cdata_y_comillas() {
        let limits = Limits {
            max_xml_depth: 3,
            max_xml_namespaces: 2,
            ..Limits::DEFAULT
        };
        let fine = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" a='x > y xmlns:p="1"'>
  <!-- <d:a><d:b><d:c><d:e> xmlns:q="2" -->
  <d:response b="" c = 'xmlns:r="3"'>
    <d:href><![CDATA[<d:a><d:b><d:c>]]></d:href>
    <d:x/><d:y /><d:z w="/"/>
    <?proceso <d:a><d:b> ?>
  </d:response>
  <d:response xmlnsx="no-es-un-espacio"><d:x/></d:response>
</d:multistatus>"#;
        assert_eq!(check_shape(fine, &limits), Ok(()));
        assert!(parse_xml(fine, &limits).is_ok());

        let deep = r#"<d:multistatus xmlns:d="DAV:"><d:a><d:b><d:c/></d:b></a></d:multistatus>"#;
        assert_eq!(check_shape(deep, &limits), Ok(()));
        let deeper =
            r#"<d:multistatus xmlns:d="DAV:"><d:a><d:b><d:c></d:c></d:b></d:a></d:multistatus>"#;
        assert!(check_shape(deeper, &limits).is_err());

        let many = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:c" xmlns = "urn:x"/>"#;
        assert!(check_shape(many, &limits).is_err());
        // Una DTD es lo que abre las entidades externas, y `roxmltree` la
        // rechazaría igual: se rechaza antes, sin llegar a armarla.
        assert!(check_shape("<!DOCTYPE d><d/>", &limits).is_err());
        for broken in ["<a", "<a b='x>", "<!-- sin cerrar", "<![CDATA[ x", "<? x"] {
            assert!(check_shape(broken, &limits).is_err(), "{broken}");
        }
    }

    /// **Miles de espacios de nombres no traban la aplicación.** Una raíz que
    /// declara cinco mil prefijos y cinco mil hijos que declaran uno cada uno
    /// son doscientos kilobytes, y `roxmltree` tarda más de dos minutos en
    /// leerlos: copia los que están a la vista en cada elemento que declara uno.
    /// Tiene que rechazarse enseguida.
    #[test]
    fn un_xml_con_miles_de_espacios_de_nombres_se_rechaza_enseguida() {
        let n = 5000;
        let root: String = (0..n).map(|i| format!(r#" xmlns:p{i}="u{i}""#)).collect();
        let children = r#"<a xmlns:z="q"/>"#.repeat(n);
        let xml = format!(r#"<d:multistatus xmlns:d="DAV:"{root}>{children}</d:multistatus>"#);

        let result = finishes_within(std::time::Duration::from_secs(5), move || {
            parse_xml(&xml, &Limits::DEFAULT).map(|_| ())
        });
        assert!(matches!(result, Err(DavError::BadXml(_))), "{result:?}");
    }

    /// Y dentro del tope de distintos, lo que cuesta es cuántos elementos
    /// declaran con muchos a la vista: treinta en la raíz y cuatrocientos mil
    /// hijos que declaran uno son casi siete megas —dentro del tope del cuerpo,
    /// que acá son ocho— y segundos de `roxmltree` en release, más en
    /// depuración, por cada respuesta. También se rechaza enseguida.
    #[test]
    fn muchos_elementos_que_declaran_con_muchos_a_la_vista_se_rechazan() {
        let root: String = (0..30).map(|i| format!(r#" xmlns:p{i}="u{i}""#)).collect();
        let children = r#"<a xmlns:z="q"/>"#.repeat(400_000);
        let xml = format!(r#"<d:multistatus xmlns:d="DAV:"{root}>{children}</d:multistatus>"#);
        assert!(xml.len() < Limits::DEFAULT.max_body_bytes);

        let result = finishes_within(std::time::Duration::from_secs(5), move || {
            parse_xml(&xml, &Limits::DEFAULT).map(|_| ())
        });
        assert!(matches!(result, Err(DavError::BadXml(_))), "{result:?}");
    }

    /// Y la misma declaración repetida en cada elemento —hay servidores que
    /// ponen `xmlns="DAV:"` en todos— no cuenta de más: a la vista sigue
    /// habiendo una. Es la razón de que el tope sea de **distintos**.
    #[test]
    fn una_declaracion_repetida_en_cada_elemento_no_cuenta_de_mas() {
        let responses =
            r#"<response xmlns="DAV:"><href xmlns="DAV:">/a.vcf</href></response>"#.repeat(20_000);
        let xml = format!(r#"<multistatus xmlns="DAV:">{responses}</multistatus>"#);

        let count = finishes_within(std::time::Duration::from_secs(10), move || {
            parse_xml(&xml, &Limits::DEFAULT)
                .map(|d| {
                    d.descendants()
                        .filter(|n| n.has_tag_name(("DAV:", "response")))
                        .count()
                })
                .unwrap_or(0)
        });
        assert_eq!(count, 20_000);
    }

    /// **Miles de atributos en un elemento no traban la aplicación.**
    /// `roxmltree` busca el atributo repetido comparando cada uno con todos los
    /// anteriores del elemento: sesenta mil atributos vacíos en el `multistatus`
    /// son menos de seiscientos kilobytes y siete segundos en release —mucho más
    /// en depuración—, y con el tope del cuerpo, horas de CPU en un hilo que no
    /// se cancela. Tiene que rechazarse enseguida.
    #[test]
    fn un_elemento_con_miles_de_atributos_se_rechaza_enseguida() {
        let xml = with_attributes(0, 60_000, " ");
        assert!(xml.len() < 600 * 1024);

        let result = finishes_within(std::time::Duration::from_secs(5), move || {
            parse_xml(&xml, &Limits::DEFAULT).map(|_| ())
        });
        assert!(matches!(result, Err(DavError::BadXml(_))), "{result:?}");
    }

    /// El tope de atributos es justo, por elemento, y cuenta las declaraciones
    /// `xmlns` y los atributos pegados sin espacio.
    #[test]
    fn el_tope_de_atributos_es_justo_y_cuenta_los_xmlns_y_los_pegados() {
        let limit = Limits::DEFAULT.max_xml_attributes;
        assert_eq!(limit, 64);

        // `xmlns:d` más 63 atributos: 64 pasan, 65 no.
        let fine = with_attributes(0, limit - 1, " ");
        assert_eq!(check_shape(&fine, &Limits::DEFAULT), Ok(()));
        assert!(parse_xml(&fine, &Limits::DEFAULT).is_ok());
        let over = with_attributes(0, limit, " ");
        assert_eq!(
            check_shape(&over, &Limits::DEFAULT),
            Err(TOO_MANY_ATTRIBUTES)
        );
        assert!(matches!(
            parse_xml(&over, &Limits::DEFAULT),
            Err(DavError::BadXml(_))
        ));

        // Las declaraciones cuentan: 1 + 30 `xmlns` + 33 atributos son 64.
        let declared = with_attributes(30, limit - 31, " ");
        assert_eq!(check_shape(&declared, &Limits::DEFAULT), Ok(()));
        assert!(parse_xml(&declared, &Limits::DEFAULT).is_ok());
        let declared_over = with_attributes(30, limit - 30, " ");
        assert_eq!(
            check_shape(&declared_over, &Limits::DEFAULT),
            Err(TOO_MANY_ATTRIBUTES)
        );

        // Y los pegados sin espacio también, `xmlns` incluido.
        let glued = with_attributes(0, limit - 1, "");
        assert_eq!(check_shape(&glued, &Limits::DEFAULT), Ok(()));
        let glued_over = with_attributes(0, limit, "");
        assert_eq!(
            check_shape(&glued_over, &Limits::DEFAULT),
            Err(TOO_MANY_ATTRIBUTES)
        );
    }

    /// El tope de espacios de nombres también es justo.
    #[test]
    fn el_tope_de_espacios_de_nombres_es_justo() {
        let limit = Limits::DEFAULT.max_xml_namespaces;
        assert_eq!(limit, 32);

        // `xmlns:d` más 31 declaraciones: 32 pasan.
        let fine = with_attributes(limit - 1, 0, " ");
        assert_eq!(check_shape(&fine, &Limits::DEFAULT), Ok(()));
        let over = with_attributes(limit, 0, " ");
        assert!(check_shape(&over, &Limits::DEFAULT).is_err());
    }

    /// Un XML roto es un error con su nombre, y el detalle recortado: el
    /// mensaje de `roxmltree` lleva los nombres de etiquetas y prefijos del
    /// documento, que pueden ser ocho megas cada uno.
    #[test]
    fn un_xml_roto_da_un_error_con_el_detalle_recortado() {
        let error = parse_xml("<a><b></a>", &Limits::DEFAULT).unwrap_err();
        assert!(matches!(error, DavError::BadXml(_)), "{error:?}");
        match error {
            DavError::BadXml(detalle) => {
                assert!(!detalle.is_empty());
                assert!(detalle.len() <= MAX_ERROR_DETAIL_BYTES, "{detalle}");
            }
            otro => panic!("{otro:?}"),
        }
    }

    /// Y **el texto que ve la persona no lleva nada del servidor**: el `Display`
    /// de cada error es fijo, sin dirección y sin lo que escribió el otro lado.
    /// El estado lo lee cualquiera de la sesión.
    ///
    /// Cada par es un error y **algo que el servidor puso adentro**: un nombre de
    /// etiqueta del documento que no se entiende, la dirección de la máquina a la
    /// que no se pudo hablar, el texto de una página de error. Si alguno aparece
    /// en el mensaje, el error filtra.
    ///
    /// Un caso por aserción y con el fragmento en el mensaje, en vez de cuatro
    /// aserciones que imprimen el texto entero: el fallo dice **qué** se escapó.
    #[test]
    fn el_texto_que_ve_la_persona_no_lleva_nada_del_servidor() {
        let casos = [
            (
                parse_xml("<a><zafiro></a>", &Limits::DEFAULT).unwrap_err(),
                "zafiro",
            ),
            (
                DavError::network("https://nube.interna.ejemplo.com/x"),
                "nube.interna.ejemplo.com",
            ),
            (
                DavError::bad_xml("tu cuenta fue suspendida, entrá a <b>este</b> sitio"),
                "suspendida",
            ),
            // Y los de estado, que son los que más tentados están a llevar la
            // dirección adentro.
            (DavError::Unauthorized, "http"),
            (DavError::Status(503), "http"),
            (DavError::Redirect(302), "http"),
            (DavError::ForeignOrigin, "http"),
            (DavError::InsecureUrl, "http"),
        ];

        for (error, fragmento) in casos {
            assert!(
                !error.to_string().contains(fragmento),
                "{fragmento:?} se escapa al texto del error: {error:?}",
            );
        }
    }

    /// El tope del cuerpo se decide una vez, y el cliente no se arma sin él.
    #[test]
    fn los_topes_son_los_del_sincronizador() {
        // Los mismos números que en `vasak-accounts`, que los usa para las mismas
        // respuestas de DAV: que sean los dos lo que hace comparables las dos
        // auditorías.
        assert_eq!(Limits::DEFAULT.max_xml_depth, 64);
        assert_eq!(Limits::DEFAULT.max_xml_namespaces, 32);
        assert_eq!(Limits::DEFAULT.max_xml_attributes, 64);
        assert_eq!(Limits::DEFAULT.max_xml_nodes, 1_000_000);
        // Y el del cuerpo, que acá es el de CardDAV: una agenda entera entra.
        assert_eq!(Limits::DEFAULT.max_body_bytes, 16 * 1024 * 1024);
    }
}
