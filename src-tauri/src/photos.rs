//! Las fotos que están afuera de la tarjeta: bajarlas una vez y guardarlas.
//!
//! ── Por qué las baja el programa y no la ventana ────────────────────────────
//!
//! Google no manda la foto adentro de la tarjeta: manda una dirección,
//! `PHOTO:https://lh3.googleusercontent.com/…`. Si la pidiera la ventana, cada
//! vez que se abre la ficha el dueño de esa dirección se enteraría —con las
//! cabeceras, las cookies y el `Referer` del motor del navegador— de que se
//! abrió, que es exactamente un rastreador de apertura como el de un correo.
//!
//! Así que la baja el programa, **una sola vez**, y la guarda en la caché de la
//! persona. A partir de ahí la ficha la muestra siempre desde el disco: quien
//! aloja la foto se entera, como mucho, de que alguien la bajó una vez, y no de
//! cuándo se mira.
//!
//! ── Cómo se baja ────────────────────────────────────────────────────────────
//!
//! - **Sólo `https`.** Por `http` la foto la ve y la puede cambiar cualquiera
//!   en el medio, y el pedido lleva la dirección entera a la vista. Una tarjeta
//!   con una foto `http` se muestra con las iniciales: ver
//!   `vcard::photo::PhotoSkipped::Insecure`.
//! - **Sin cookies, sin `Referer`, sin redirecciones**, con plazo y con tope: el
//!   mismo [`MAX_PHOTO_BYTES`] de las fotos que vienen adentro. Sin
//!   redirecciones porque cada salto es una dirección más que nadie eligió, y
//!   porque es cómo una dirección pública termina apuntando adentro de la red.
//! - **Sólo a direcciones públicas.** La tarjeta la escribió cualquiera, y una
//!   `PHOTO:https://192.168.0.1/…` haría que esta aplicación le pegara al router
//!   de la persona. Se mira el número si la dirección es un número, y lo que
//!   devuelve el DNS si es un nombre: un nombre público que resuelve a una
//!   dirección privada no se sigue.
//! - **Tiene que ser una imagen de verdad**: el `Content-Type` tiene que decir
//!   que lo es, y los primeros bytes también. Con uno solo no alcanza —un
//!   servidor miente en la cabecera tan fácil como en el contenido—.
//!
//! ── Cómo se guarda ──────────────────────────────────────────────────────────
//!
//! En `$XDG_CACHE_HOME/vasak-contacts/photos/`, con el nombre que da el SHA-256
//! de la dirección: la dirección no queda escrita en el disco, y una tarjeta que
//! cambia de foto cambia de dirección y **se baja la nueva**. Se escribe a un
//! archivo aparte y se renombra, así que una lectura nunca ve una foto a medias.
//!
//! Si falla, queda una marca y **no se vuelve a intentar hasta dentro de
//! [`RETRY_AFTER`]**: una foto que no está no puede hacer que cada vez que se
//! abre la ficha se salga a buscarla.
//!
//! ── Hasta cuándo es así ─────────────────────────────────────────────────────
//!
//! Esta caché es provisoria. Cuando exista el servicio del usuario que mantiene
//! la copia local de correo, calendarios y contactos (decisión 6 del taller,
//! Vasak-OS/vasak-calendar#11 y Vasak-OS/vasak-accounts#23), las fotos pasan a
//! ese almacén y esto se va.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, SystemTime};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use sha2::{Digest, Sha256};
use url::Url;

use crate::vcard::photo::{data_uri, sniff_image, MAX_PHOTO_BYTES};

/// Cuánto se espera antes de volver a intentar una foto que no se pudo bajar.
pub const RETRY_AFTER: Duration = Duration::from_secs(6 * 60 * 60);

/// El plazo entero de una descarga. Una foto de 256 KiB tarda menos de un
/// segundo en cualquier conexión que se pueda usar; diez son para no cortar la
/// que va lenta, no para esperar a un servidor que no contesta.
const TIMEOUT: Duration = Duration::from_secs(10);

/// El largo máximo de una dirección que se acepta: la de Google tiene unos
/// doscientos caracteres, y una de diez mil no es una foto.
const MAX_URL_BYTES: usize = 2048;

/// Los tipos que se aceptan en el `Content-Type`.
const IMAGE_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif", "image/webp"];

/// Por qué no se pudo bajar una foto. No sale nunca a la ventana: una foto que
/// no está se ve como las iniciales, y el motivo sirve para las pruebas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// No es una dirección `https` que se pueda pedir.
    NotAllowed,
    /// Apunta adentro de la red: a esta máquina, al router, a una red privada.
    PrivateAddress,
    Status(u16),
    /// El servidor no dijo que fuera una imagen, o los bytes no lo son.
    NotAnImage,
    TooLarge,
    Network,
}

/// Cómo se baja: lo que cambia entre la aplicación y las pruebas.
#[derive(Debug, Clone)]
pub struct FetchPolicy {
    /// **Sólo para las pruebas**: dejar `http` hacia esta misma máquina, que es
    /// donde corre el servidor de prueba. La aplicación nunca lo tiene.
    allow_loopback_http: bool,
    timeout: Duration,
    retry_after: Duration,
}

impl FetchPolicy {
    pub fn production() -> Self {
        FetchPolicy {
            allow_loopback_http: false,
            timeout: TIMEOUT,
            retry_after: RETRY_AFTER,
        }
    }
}

/// Una sola descarga por dirección a la vez: si la ficha se abre dos veces
/// seguidas mientras la foto baja, la segunda espera a la primera y lee de la
/// caché, en vez de bajarla de nuevo.
static IN_FLIGHT: LazyLock<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Default::default);

/// La carpeta de la caché de fotos, o `None` si la sesión no tiene una.
///
/// `dirs::cache_dir()` ya hace lo que dice el estándar de XDG: usa
/// `XDG_CACHE_HOME` si es una ruta absoluta y cae a `~/.cache` si no.
pub fn default_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("vasak-contacts").join("photos"))
}

/// El nombre en disco de una dirección: su SHA-256 en hexadecimal.
pub fn key_for(url: &str) -> String {
    Sha256::digest(url.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// La caché de fotos de una carpeta.
pub struct PhotoStore {
    dir: PathBuf,
    policy: FetchPolicy,
}

impl PhotoStore {
    pub fn new(dir: PathBuf, policy: FetchPolicy) -> Self {
        PhotoStore { dir, policy }
    }

    fn image_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.img"))
    }

    fn failure_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.failed"))
    }

    /// La foto de una dirección como `data:` URI: de la caché si ya está, y si
    /// no, bajándola y guardándola. `None` si no se pudo, y entonces no se
    /// vuelve a intentar hasta dentro de un rato.
    pub async fn photo(&self, url: &str) -> Option<String> {
        self.photo_with_reason(url).await.ok()
    }

    /// Lo mismo que [`PhotoStore::photo`], con el motivo cuando falla.
    pub async fn photo_with_reason(&self, url: &str) -> Result<String, FetchError> {
        let key = key_for(url);

        if let Some(uri) = self.cached(&key).await {
            return Ok(uri);
        }

        let lock = {
            let mut map = IN_FLIGHT
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.entry(key.clone()).or_default().clone()
        };
        let outcome = {
            let _guard = lock.lock().await;
            self.fetch_and_store(url, &key).await
        };

        // La entrada se saca siempre, salga como salga: si no, cada dirección
        // que se pidió alguna vez quedaría en memoria hasta cerrar.
        IN_FLIGHT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&key);
        outcome
    }

    async fn fetch_and_store(&self, url: &str, key: &str) -> Result<String, FetchError> {
        // Otra descarga de la misma foto pudo haber terminado mientras se
        // esperaba el turno.
        if let Some(uri) = self.cached(key).await {
            return Ok(uri);
        }
        if self.failed_recently(key).await {
            return Err(FetchError::Network);
        }

        match fetch_photo(url, &self.policy).await {
            Ok((mime, bytes)) => {
                // Si no se puede guardar, igual se muestra: la próxima vez se
                // vuelve a bajar, que es peor pero no roto.
                let _ = self.store(key, &bytes).await;
                Ok(data_uri(mime, &bytes))
            }
            Err(e) => {
                let _ = self.mark_failed(key).await;
                Err(e)
            }
        }
    }

    /// La foto guardada, si está y sigue siendo una imagen que entra en el
    /// tope. Se vuelve a mirar al leer: la caché es un archivo en el disco de
    /// la persona, y cualquier otro programa de la sesión puede cambiarlo.
    async fn cached(&self, key: &str) -> Option<String> {
        let bytes = tokio::fs::read(self.image_path(key)).await.ok()?;
        if bytes.len() > MAX_PHOTO_BYTES {
            return None;
        }
        let mime = sniff_image(&bytes)?;
        Some(data_uri(mime, &bytes))
    }

    async fn failed_recently(&self, key: &str) -> bool {
        let Ok(meta) = tokio::fs::metadata(self.failure_path(key)).await else {
            return false;
        };
        let Ok(when) = meta.modified() else {
            return false;
        };
        // Una marca del futuro —un reloj que se corrió— cuenta como reciente
        // hasta que se la alcance, en vez de volver a intentar en cada apertura.
        SystemTime::now()
            .duration_since(when)
            .map_or(true, |age| age < self.policy.retry_after)
    }

    async fn ensure_dir(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        // Las fotos de los contactos de alguien son de ese alguien.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700)).await?;
        }
        Ok(())
    }

    /// Guarda **de una vez**: se escribe a un archivo aparte y se renombra. Una
    /// lectura que llegue en el medio encuentra la foto vieja o ninguna, nunca
    /// media.
    async fn store(&self, key: &str, bytes: &[u8]) -> std::io::Result<()> {
        self.ensure_dir().await?;
        let temporary = self.dir.join(format!(
            ".{key}.{}.{}.tmp",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        write_private(&temporary, bytes).await?;
        if let Err(e) = tokio::fs::rename(&temporary, self.image_path(key)).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(e);
        }
        let _ = tokio::fs::remove_file(self.failure_path(key)).await;
        Ok(())
    }

    async fn mark_failed(&self, key: &str) -> std::io::Result<()> {
        self.ensure_dir().await?;
        write_private(&self.failure_path(key), b"").await
    }
}

async fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path).await?;
    file.write_all(bytes).await?;
    file.sync_all().await
}

/// Si una dirección es de internet y no de adentro de la red.
///
/// Afuera: esta máquina, las redes privadas, las de enlace local (donde vive el
/// servicio de metadatos de cualquier nube, `169.254.169.254`), la de los
/// operadores (`100.64.0.0/10`), las de documentación, multidifusión y la
/// dirección sin especificar. Y una IPv6 que envuelve una IPv4 se mira por la
/// IPv4 que lleva adentro, o `::ffff:127.0.0.1` se colaría.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && v4.octets()[2] == 0)
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let s = v6.segments();
            let first = s[0];
            let embedded = |a: u16, b: u16| {
                IpAddr::V4(std::net::Ipv4Addr::new(
                    (a >> 8) as u8,
                    a as u8,
                    (b >> 8) as u8,
                    b as u8,
                ))
            };
            // Las que llevan una IPv4 adentro se miran por esa IPv4: si no, un
            // traductor NAT64 o un túnel 6to4 llevan a la red de la casa.
            if first == 0x0064 && s[1] == 0xff9b {
                // 64:ff9b:1::/48 es de uso local, y no hay una sola forma de
                // saber dónde va la IPv4: se rechaza entera.
                if s[2] == 1 {
                    return false;
                }
                // 64:ff9b::/96, la de NAT64.
                if s[2..6].iter().all(|x| *x == 0) {
                    return is_public(embedded(s[6], s[7]));
                }
            }
            // 2002::/16, 6to4: la IPv4 va en los dos segmentos siguientes.
            if first == 0x2002 {
                return is_public(embedded(s[1], s[2]));
            }
            // `::a.b.c.d`, las compatibles con IPv4, ya en desuso: también
            // cubren `::1` y `::`.
            if s[..6].iter().all(|x| *x == 0) {
                return false;
            }
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7, las locales únicas.
                || (first & 0xfe00) == 0xfc00
                // fe80::/10, las de enlace local.
                || (first & 0xffc0) == 0xfe80
                // 2001:db8::/32, las de documentación.
                || (first == 0x2001 && s[1] == 0x0db8))
        }
    }
}

/// El DNS de las descargas: el del sistema, pero **sin** las direcciones que
/// apuntan adentro de la red.
struct PublicOnlyResolver {
    allow_loopback: bool,
}

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let allow_loopback = self.allow_loopback;
        Box::pin(async move {
            let found: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| is_public(a.ip()) || (allow_loopback && a.ip().is_loopback()))
                .collect();
            if found.is_empty() {
                return Err("el nombre sólo resuelve a direcciones de adentro de la red".into());
            }
            let addrs: Addrs = Box::new(found.into_iter());
            Ok(addrs)
        })
    }
}

/// Comprueba la dirección antes de pedirla.
fn check_url(url: &str, policy: &FetchPolicy) -> Result<Url, FetchError> {
    if url.len() > MAX_URL_BYTES {
        return Err(FetchError::NotAllowed);
    }
    let parsed = Url::parse(url).map_err(|_| FetchError::NotAllowed)?;
    // Una dirección con usuario y contraseña adentro manda una credencial que
    // escribió quien hizo la tarjeta.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(FetchError::NotAllowed);
    }
    let host = parsed.host().ok_or(FetchError::NotAllowed)?;
    let loopback = match &host {
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
        url::Host::Domain(d) => d.eq_ignore_ascii_case("localhost"),
    };

    match parsed.scheme() {
        "https" => {}
        "http" if policy.allow_loopback_http && loopback => {}
        _ => return Err(FetchError::NotAllowed),
    }

    let literal = match host {
        url::Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
        url::Host::Domain(d) => {
            // `localhost` y sus subdominios los resuelve el sistema a esta
            // máquina sin preguntarle a nadie.
            let d = d.to_ascii_lowercase();
            if (d == "localhost" || d.ends_with(".localhost")) && !policy.allow_loopback_http {
                return Err(FetchError::PrivateAddress);
            }
            None
        }
    };
    if let Some(ip) = literal {
        if !is_public(ip) && !(policy.allow_loopback_http && ip.is_loopback()) {
            return Err(FetchError::PrivateAddress);
        }
    }
    Ok(parsed)
}

/// Baja una foto, con todos los controles. Devuelve el tipo que dicen sus
/// bytes y los bytes.
pub async fn fetch_photo(
    url: &str,
    policy: &FetchPolicy,
) -> Result<(&'static str, Vec<u8>), FetchError> {
    let parsed = check_url(url, policy)?;

    let client = reqwest::Client::builder()
        .https_only(!policy.allow_loopback_http)
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .timeout(policy.timeout)
        .connect_timeout(policy.timeout)
        .dns_resolver(Arc::new(PublicOnlyResolver {
            allow_loopback: policy.allow_loopback_http,
        }))
        .build()
        .map_err(|_| FetchError::Network)?;

    let mut response = client
        .get(parsed)
        .header(reqwest::header::ACCEPT, IMAGE_TYPES.join(", "))
        .send()
        .await
        .map_err(|e| {
            // El resolvedor que rechaza direcciones privadas lo hace como un
            // error de conexión; se distingue para que las pruebas lo vean.
            let chain =
                std::iter::successors(Some(&e as &(dyn std::error::Error + 'static)), |e| {
                    e.source()
                });
            if chain
                .map(ToString::to_string)
                .any(|m| m.contains("adentro de la red"))
            {
                FetchError::PrivateAddress
            } else {
                FetchError::Network
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(FetchError::Status(status.as_u16()));
    }

    let declared = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    if !IMAGE_TYPES.contains(&declared.as_str()) {
        return Err(FetchError::NotAnImage);
    }

    if response
        .content_length()
        .is_some_and(|len| len > MAX_PHOTO_BYTES as u64)
    {
        return Err(FetchError::TooLarge);
    }

    // El largo declarado puede faltar o mentir: se cuenta lo que llega.
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| FetchError::Network)? {
        if bytes.len() + chunk.len() > MAX_PHOTO_BYTES {
            return Err(FetchError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }

    let mime = sniff_image(&bytes).ok_or(FetchError::NotAnImage)?;
    Ok((mime, bytes))
}

/// La foto de un contacto que está afuera de la tarjeta, desde la caché.
///
/// La ventana la pide con la dirección que vino en `Contact.photo_url`, y
/// recibe una `data:` URI o nada. **Nunca** recibe la dirección para pedirla
/// ella.
#[tauri::command]
pub async fn contact_photo(url: String) -> Result<Option<String>, String> {
    let Some(dir) = default_cache_dir() else {
        return Ok(None);
    };
    Ok(PhotoStore::new(dir, FetchPolicy::production())
        .photo(&url)
        .await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcard::photo::tests::{jpeg_of, PNG_1X1};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Un servidor de prueba en esta máquina, **sin salir a internet**. Anota
    /// cuántos pedidos le llegaron y las cabeceras de cada uno.
    struct TestServer {
        base: String,
        hits: Arc<AtomicUsize>,
        requests: Arc<Mutex<Vec<String>>>,
    }

    async fn serve() -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (h, r) = (hits.clone(), requests.clone());

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let (h, r) = (h.clone(), r.clone());
                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 8192];
                    let mut read = 0;
                    loop {
                        let Ok(n) = socket.read(&mut buffer[read..]).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        read += n;
                        if buffer[..read].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    h.fetch_add(1, Ordering::SeqCst);
                    r.lock().unwrap().push(request.clone());
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();

                    let (status, content_type, body, length): (&str, &str, Vec<u8>, bool) =
                        match path.as_str() {
                            "/ana.png" | "/otra.png" => {
                                ("200 OK", "image/png", PNG_1X1.to_vec(), true)
                            }
                            "/ana.jpg" => {
                                ("200 OK", "image/jpeg; charset=binary", jpeg_of(1024), true)
                            }
                            "/enorme.jpg" => {
                                ("200 OK", "image/jpeg", jpeg_of(MAX_PHOTO_BYTES + 1), true)
                            }
                            "/enorme-sin-largo.jpg" => {
                                ("200 OK", "image/jpeg", jpeg_of(MAX_PHOTO_BYTES + 1), false)
                            }
                            "/pagina.png" => ("200 OK", "text/html", PNG_1X1.to_vec(), true),
                            "/mentira.png" => {
                                ("200 OK", "image/png", b"<html>hola</html>".to_vec(), true)
                            }
                            "/svg" => ("200 OK", "image/svg+xml", b"<svg/>".to_vec(), true),
                            "/lenta.png" => {
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                ("200 OK", "image/png", PNG_1X1.to_vec(), true)
                            }
                            "/salto" => ("302 Found", "text/plain", Vec::new(), true),
                            _ => ("404 Not Found", "text/plain", Vec::new(), true),
                        };
                    let mut head = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nConnection: close\r\n");
                    if path == "/salto" {
                        head.push_str("Location: /ana.png\r\n");
                    }
                    if length {
                        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
                    }
                    // Una cookie que el cliente no tiene que devolver nunca.
                    head.push_str("Set-Cookie: rastreo=1; Path=/\r\n\r\n");
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                    let _ = socket.shutdown().await;
                });
            }
        });

        TestServer {
            base,
            hits,
            requests,
        }
    }

    fn test_policy() -> FetchPolicy {
        FetchPolicy {
            allow_loopback_http: true,
            timeout: Duration::from_secs(1),
            retry_after: RETRY_AFTER,
        }
    }

    /// Una carpeta de caché propia de cada prueba, bajo el directorio
    /// temporal (`TMPDIR`): nada que toque la caché de verdad de la persona.
    fn cache_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("vasak-contacts-photo-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// **Se baja una sola vez**: la segunda vez sale de la caché y el servidor
    /// no se entera.
    #[tokio::test]
    async fn la_foto_se_baja_una_sola_vez() {
        let server = serve().await;
        let store = PhotoStore::new(cache_dir("una-vez"), test_policy());
        let url = format!("{}/ana.png", server.base);

        let first = store.photo(&url).await.expect("se bajó");
        assert!(first.starts_with("data:image/png;base64,"));
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);

        let second = store.photo(&url).await.expect("de la caché");
        assert_eq!(first, second);
        assert_eq!(server.hits.load(Ordering::SeqCst), 1, "se volvió a pedir");

        // Y otra caché sobre la misma carpeta —la próxima vez que se abre la
        // aplicación— también la encuentra.
        let later = PhotoStore::new(store.dir.clone(), test_policy());
        assert_eq!(later.photo(&url).await, Some(first));
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    }

    /// **Sin cookies y sin `Referer`**: el servidor manda una cookie, y el
    /// pedido siguiente no la devuelve; ninguno lleva de dónde viene.
    #[tokio::test]
    async fn el_pedido_no_lleva_cookies_ni_referer() {
        let server = serve().await;
        let store = PhotoStore::new(cache_dir("cabeceras"), test_policy());
        store
            .photo(&format!("{}/ana.png", server.base))
            .await
            .unwrap();
        store
            .photo(&format!("{}/otra.png", server.base))
            .await
            .unwrap();

        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        for request in requests.iter() {
            let lower = request.to_ascii_lowercase();
            assert!(!lower.contains("\r\ncookie:"), "{request}");
            assert!(!lower.contains("\r\nreferer:"), "{request}");
            assert!(lower.starts_with("get "), "{request}");
        }
    }

    /// Una tarjeta que cambia de foto cambia de dirección, y **se baja la
    /// nueva**: la clave es la dirección.
    #[tokio::test]
    async fn si_la_tarjeta_cambia_de_url_se_baja_la_nueva() {
        let server = serve().await;
        let store = PhotoStore::new(cache_dir("cambia"), test_policy());

        let png = store
            .photo(&format!("{}/ana.png", server.base))
            .await
            .unwrap();
        let jpg = store
            .photo(&format!("{}/ana.jpg", server.base))
            .await
            .unwrap();
        assert_eq!(server.hits.load(Ordering::SeqCst), 2);
        assert!(png.starts_with("data:image/png"));
        assert!(jpg.starts_with("data:image/jpeg"));
        assert_ne!(key_for("https://a/1"), key_for("https://a/2"));
    }

    /// La caché se guarda con el nombre del hash, **sin la dirección**, con
    /// permisos de la persona sola y sin temporales que queden tirados.
    #[tokio::test]
    async fn la_cache_se_guarda_con_el_hash_y_sin_temporales() {
        let server = serve().await;
        let dir = cache_dir("disco");
        let store = PhotoStore::new(dir.clone(), test_policy());
        let url = format!("{}/ana.png", server.base);
        store.photo(&url).await.unwrap();

        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec![format!("{}.img", key_for(&url))]);
        assert_eq!(std::fs::read(dir.join(&names[0])).unwrap(), PNG_1X1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(&dir.join(&names[0])), 0o600);
        }
    }

    /// Una foto guardada que alguien cambió por algo que no es una imagen no se
    /// muestra: se vuelve a mirar al leer.
    #[tokio::test]
    async fn una_cache_adulterada_no_se_muestra() {
        let server = serve().await;
        let dir = cache_dir("adulterada");
        let store = PhotoStore::new(dir.clone(), test_policy());
        let url = format!("{}/ana.png", server.base);
        store.photo(&url).await.unwrap();

        std::fs::write(dir.join(format!("{}.img", key_for(&url))), b"<script>").unwrap();
        // Se descarta y se vuelve a bajar.
        let again = store.photo(&url).await.unwrap();
        assert!(again.starts_with("data:image/png"));
        assert_eq!(server.hits.load(Ordering::SeqCst), 2);
    }

    /// **Tiene que ser una imagen de verdad**: por la cabecera **y** por los
    /// bytes.
    #[tokio::test]
    async fn tiene_que_ser_una_imagen_por_la_cabecera_y_por_los_bytes() {
        let server = serve().await;
        let policy = test_policy();
        for (path, expected) in [
            ("/pagina.png", FetchError::NotAnImage),
            ("/mentira.png", FetchError::NotAnImage),
            ("/svg", FetchError::NotAnImage),
            ("/no-existe", FetchError::Status(404)),
        ] {
            let got = fetch_photo(&format!("{}{path}", server.base), &policy).await;
            assert_eq!(got.map(|(m, _)| m), Err(expected), "{path}");
        }
    }

    /// **El tope es el de la foto**, con el largo declarado y sin él: el
    /// servidor puede no decirlo, o mentirlo.
    #[tokio::test]
    async fn una_foto_remota_enorme_supera_el_tope() {
        let server = serve().await;
        let policy = test_policy();
        for path in ["/enorme.jpg", "/enorme-sin-largo.jpg"] {
            let got = fetch_photo(&format!("{}{path}", server.base), &policy).await;
            assert_eq!(got.map(|(m, _)| m), Err(FetchError::TooLarge), "{path}");
        }
    }

    /// Sin redirecciones: cada salto es una dirección que nadie eligió.
    #[tokio::test]
    async fn no_se_siguen_las_redirecciones() {
        let server = serve().await;
        let got = fetch_photo(&format!("{}/salto", server.base), &test_policy()).await;
        assert_eq!(got.map(|(m, _)| m), Err(FetchError::Status(302)));
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    }

    /// Un servidor que no contesta no cuelga la ficha: hay plazo.
    #[tokio::test]
    async fn un_servidor_lento_se_corta_por_plazo() {
        let server = serve().await;
        let started = std::time::Instant::now();
        let got = fetch_photo(&format!("{}/lenta.png", server.base), &test_policy()).await;
        assert_eq!(got.map(|(m, _)| m), Err(FetchError::Network));
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "{:?}",
            started.elapsed()
        );
    }

    /// **Si falla, no se reintenta en cada apertura**: queda una marca, y hasta
    /// que pase el rato no se sale a buscarla. Pasado el rato, sí.
    #[tokio::test]
    async fn si_falla_se_reintenta_mas_adelante_y_no_en_cada_apertura() {
        let server = serve().await;
        let dir = cache_dir("reintento");
        let url = format!("{}/no-existe", server.base);

        let store = PhotoStore::new(dir.clone(), test_policy());
        assert_eq!(store.photo(&url).await, None);
        assert_eq!(store.photo(&url).await, None);
        assert_eq!(store.photo(&url).await, None);
        assert_eq!(
            server.hits.load(Ordering::SeqCst),
            1,
            "se reintentó enseguida"
        );
        assert!(dir.join(format!("{}.failed", key_for(&url))).exists());

        // Con el rato cumplido, se vuelve a intentar.
        let later = PhotoStore::new(
            dir,
            FetchPolicy {
                retry_after: Duration::ZERO,
                ..test_policy()
            },
        );
        assert_eq!(later.photo(&url).await, None);
        assert_eq!(server.hits.load(Ordering::SeqCst), 2);
    }

    /// Y cuando la foto por fin se baja, la marca se va.
    #[tokio::test]
    async fn cuando_se_baja_la_marca_de_fallo_se_va() {
        let server = serve().await;
        let dir = cache_dir("marca");
        let url = format!("{}/ana.png", server.base);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{}.failed", key_for(&url))), b"").unwrap();

        let store = PhotoStore::new(
            dir.clone(),
            FetchPolicy {
                retry_after: Duration::ZERO,
                ..test_policy()
            },
        );
        assert!(store.photo(&url).await.is_some());
        assert!(!dir.join(format!("{}.failed", key_for(&url))).exists());
    }

    /// Dos aperturas seguidas mientras la foto baja: **una sola descarga**.
    #[tokio::test]
    async fn dos_pedidos_a_la_vez_bajan_una_sola_vez() {
        let server = serve().await;
        let store = Arc::new(PhotoStore::new(cache_dir("a-la-vez"), test_policy()));
        let url = format!("{}/ana.jpg", server.base);
        let (a, b) = tokio::join!(store.photo(&url), store.photo(&url));
        assert!(a.is_some() && a == b);
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    }

    /// **En la aplicación, sólo `https`**, y la prueba no sale a la red: se
    /// rechaza antes de conectar.
    #[tokio::test]
    async fn en_la_aplicacion_solo_https() {
        let production = FetchPolicy::production();
        for url in [
            "http://ejemplo.com/ana.jpg",
            "ftp://ejemplo.com/ana.jpg",
            "file:///etc/passwd",
            "data:image/png;base64,AAAA",
            "https://ana:secreto@ejemplo.com/ana.jpg",
            "no es una dirección",
        ] {
            assert_eq!(
                fetch_photo(url, &production).await.map(|(m, _)| m),
                Err(FetchError::NotAllowed),
                "{url}"
            );
        }
        let larga = format!("https://ejemplo.com/{}", "a".repeat(MAX_URL_BYTES));
        assert_eq!(
            fetch_photo(&larga, &production).await.map(|(m, _)| m),
            Err(FetchError::NotAllowed)
        );
        // Y el servidor de prueba, que es `http`, en la aplicación no se pide.
        let server = serve().await;
        assert_eq!(
            fetch_photo(&format!("{}/ana.png", server.base), &production)
                .await
                .map(|(m, _)| m),
            Err(FetchError::NotAllowed)
        );
        assert_eq!(server.hits.load(Ordering::SeqCst), 0);
    }

    /// **Nada adentro de la red**: ni esta máquina, ni el router, ni el
    /// servicio de metadatos de una nube. Tampoco con el número escrito en
    /// IPv6, ni con `localhost`.
    #[tokio::test]
    async fn una_direccion_de_adentro_de_la_red_no_se_pide() {
        let production = FetchPolicy::production();
        for url in [
            "https://127.0.0.1/a.png",
            "https://192.168.0.1/a.png",
            "https://10.0.0.1/a.png",
            "https://172.16.5.4/a.png",
            "https://169.254.169.254/latest/meta-data",
            "https://100.64.0.1/a.png",
            "https://0.0.0.0/a.png",
            "https://[::1]/a.png",
            "https://[::ffff:127.0.0.1]/a.png",
            "https://[fd00::1]/a.png",
            "https://[fe80::1]/a.png",
            "https://localhost/a.png",
            "https://fotos.localhost/a.png",
        ] {
            assert_eq!(
                fetch_photo(url, &production).await.map(|(m, _)| m),
                Err(FetchError::PrivateAddress),
                "{url}"
            );
        }
    }

    /// El DNS tampoco puede llevar adentro: un nombre que resuelve a esta
    /// máquina se descarta. `localhost` resuelve sin salir a la red.
    #[tokio::test]
    async fn el_dns_no_lleva_adentro_de_la_red() {
        let resolver = PublicOnlyResolver {
            allow_loopback: false,
        };
        let name: Name = "localhost".parse().unwrap();
        assert!(resolver.resolve(name).await.is_err());

        let permissive = PublicOnlyResolver {
            allow_loopback: true,
        };
        let name: Name = "localhost".parse().unwrap();
        assert!(permissive.resolve(name).await.is_ok());
    }

    #[test]
    fn que_direcciones_son_publicas() {
        let public = |s: &str| is_public(s.parse().unwrap());
        assert!(public("142.250.80.46"));
        assert!(public("2607:f8b0:4004:c07::64"));
        // Con una IPv4 pública adentro, sí.
        assert!(public("64:ff9b::8efa:502e"));
        assert!(public("2002:8efa:502e::1"));
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.1",
            "172.31.255.255",
            "169.254.1.1",
            "100.100.100.100",
            "0.1.2.3",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.2.1",
            "192.0.0.8",
            "240.0.0.1",
            "::1",
            "::",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:10.0.0.1",
            "64:ff9b::c0a8:1",
            "64:ff9b:1::1",
            "2002:c0a8:0101::1",
            "::7f00:1",
        ] {
            assert!(!public(private), "{private}");
        }
    }

    #[test]
    fn la_clave_es_el_sha256_de_la_direccion() {
        assert_eq!(
            key_for(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(key_for("https://x/a").len(), 64);
    }

    /// La caché va donde dice XDG, dentro de una carpeta propia.
    #[test]
    fn la_cache_va_en_la_carpeta_de_xdg() {
        if let Some(dir) = default_cache_dir() {
            assert!(dir.ends_with("vasak-contacts/photos"), "{dir:?}");
            assert!(dir.is_absolute());
        }
    }
}
