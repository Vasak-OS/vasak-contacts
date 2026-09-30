# vasak-contacts

La libreta de direcciones de VasakOS. Muestra los contactos de las cuentas
conectadas —las que se agregan desde Configuración— con búsqueda.

Tauri 2 + Vue 3 + TypeScript + Tailwind 4, sobre la plantilla
[`vapp`](https://github.com/Vasak-OS/vapp).

---

## Qué hace hoy, y qué no

**Hace:** lista las cuentas con libreta, descubre las libretas de cada una por
CardDAV, trae todos los contactos y los muestra ordenados por apellido y
agrupados por inicial. Se busca por nombre, apodo, organización, cargo, correo y
teléfono. Desde un contacto se le puede escribir, llamar o copiar el dato.

La ficha muestra la foto (o las iniciales), el apodo, el cargo y la función, las
direcciones en renglones, el cumpleaños y el aniversario —también sin año—, las
webs, las redes y la mensajería, las categorías, y en «Más datos» los idiomas,
la zona horaria, la ubicación y los campos a medida (`X-`). Lo que está vacío no
se muestra.

**Todavía no hace:**

- **Crear, editar ni borrar contactos.** Escribir por CardDAV es su propio
  trabajo (Vasak-OS/vasak-contacts#2): el `If-Match`, los conflictos con lo que
  cambió otro cliente, y qué hacer cuando dos dispositivos editaron al mismo
  tiempo.
- **Las relaciones** (`RELATED`, las de Apple y las de Google) y los miembros de
  un grupo (`MEMBER`): hay que resolverlas contra los demás contactos, y es
  Vasak-OS/vasak-contacts#6.

### Las fotos

Las que vienen adentro de la tarjeta —base64 en la 2.1 y la 3.0, `data:` en la
4.0— se decodifican una vez al leer, se reconocen por sus bytes (JPEG, PNG, GIF
o WebP; un SVG no) y tienen su propio tope, 256 KiB.

Las que vienen como una dirección, que es lo que manda Google, **no las pide
nunca la ventana**: sería un rastreador de apertura. Las baja el programa una
sola vez —sólo `https`, sin cookies ni `Referer`, sin redirecciones, sólo a
direcciones públicas, con plazo y con el mismo tope—, comprueba que sean una
imagen por el `Content-Type` y por los bytes, y las guarda en
`$XDG_CACHE_HOME/vasak-contacts/photos/`. La ficha las muestra siempre desde
ahí. Si una falla, no se vuelve a intentar por seis horas. Esta caché pasa al
almacén local del servicio del usuario cuando exista (decisión 6 del taller).

---

## De dónde salen los contactos

De `vasak-accounts`, por D-Bus en el bus del sistema. **Esta aplicación no le
pide la contraseña a nadie**: la cuenta se conecta una vez desde Configuración y
acá se le pide al servicio la credencial cuando hace falta. La primera vez
aparece el diálogo de permiso del sistema; listar las cuentas no lo dispara.

### El límite

Pide **`account.contacts` y nada más**, y `vasak-permissions` lo tiene declarado
para `/usr/bin/vasak-contacts`, así que un pedido fuera de ahí se niega sin
siquiera preguntarle a la persona.

Acá importa especialmente: **los contactos y los calendarios viven en el mismo
servidor y detrás de la misma contraseña**. Un Nextcloud entrega
`/remote.php/dav/calendars/…` y `/remote.php/dav/addressbooks/…` con la misma
credencial de aplicación. Con el alcance declarado en las dos aplicaciones, el
límite queda de los dos lados: el calendario no llega a la agenda y la agenda no
llega a los eventos.

---

## Cómo está armado

| archivo | qué resuelve |
|---|---|
| `src-tauri/src/cuentas.rs` | Habla con `vasak-accounts`. `Credencial` **no deriva `Debug`** — el secreto se tacha a mano. |
| `src-tauri/src/carddav.rs` | `PROPFIND` para descubrir las libretas, `REPORT` para traer las tarjetas. |
| `src-tauri/src/vcard/` | El parseo de las tarjetas: las líneas (`lines.rs`), el contacto (`contact.rs`), las fechas, la foto y las redes. Las tres versiones del formato. |
| `src-tauri/src/photos.rs` | Las fotos que están afuera de la tarjeta: bajarlas una vez y guardarlas en la caché. |
| `src/tools/address-book.ts` | Ordenar y buscar. |
| `src/tools/contact-fields.ts` | Cómo se muestra cada campo: renglones de la dirección, fechas sin año, qué web se puede abrir. |

### Por qué se trae todo de una

Un calendario se mira por mes, así que se pide un mes. Una libreta se **busca**,
y para buscar hay que tener todo. Una agenda de mil contactos son unos pocos
megabytes de texto: se traen una vez al abrir y se busca en memoria, que es
instantáneo y no le pega al servidor con cada tecla.

### Por qué el parseo de vCard tiene tantos tests

Porque conviven tres versiones del formato y las diferencias muerden:

- En **2.1** los parámetros van sueltos (`TEL;HOME:`) y el texto puede venir en
  `quoted-printable`. La escriben los teléfonos viejos y los exportadores de
  agendas de hace veinte años — que es justo lo que la gente tiene guardado. Sin
  leerla, media agenda en español se ve con signos de igual en el medio de los
  nombres.
- En **3.0** los parámetros llevan nombre (`TEL;TYPE=HOME:`).
- En **4.0** las direcciones llevan `mailto:` adelante. Dejarlo haría que el
  botón de escribir abriera «mailto:mailto:…».

Y porque una tarjeta **la escribió cualquiera**: puede haber llegado adjunta a un
correo o importada de un teléfono. Una rota no puede impedir ver las otras
trescientas.

### Ordenar y buscar, que es lo que se equivoca callado

**El orden lo pone la ventana con `Intl.Collator`**, no el programa comparando
cadenas. Comparar por código de carácter manda «Álvarez» después de «Zaparte»,
porque la Á está por encima de cualquier letra ASCII: en español eso desordena
media agenda, y se ve prolijo.

**La búsqueda ignora los acentos.** Nadie los escribe en un buscador, ni siquiera
quien los escribe bien en todo lo demás, y una búsqueda que no los ignora
simplemente dice que no hay nadie. Los teléfonos se buscan sin los separadores,
porque nadie pone el guión en el mismo lugar.

---

## Desarrollo

```bash
bun install
bun test                                          # el frontend
cargo test --manifest-path src-tauri/Cargo.toml   # el backend
bun run lint
bunx --bun tauri dev
```

Para probar la ventana de verdad hace falta `--features custom-protocol`, o el
webview abre vacío:

```bash
bunx --bun tauri build --debug --features custom-protocol
```

Y hace falta `vasak-accounts` corriendo con al menos una cuenta que tenga
libreta. Sin eso la aplicación abre igual y lo dice.
