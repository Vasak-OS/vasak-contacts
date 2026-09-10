# vasak-contacts

La libreta de direcciones de VasakOS. Muestra los contactos de las cuentas
conectadas —las que se agregan desde Configuración— con búsqueda.

Tauri 2 + Vue 3 + TypeScript + Tailwind 4, sobre la plantilla
[`vapp`](https://github.com/Vasak-OS/vapp).

---

## Qué hace hoy, y qué no

**Hace:** lista las cuentas con libreta, descubre las libretas de cada una por
CardDAV, trae todos los contactos y los muestra ordenados por apellido y
agrupados por inicial. Se busca por nombre, organización, correo y teléfono.
Desde un contacto se le puede escribir, llamar o copiar el dato.

**Todavía no hace:**

- **Crear, editar ni borrar contactos.** Escribir por CardDAV es su propio
  trabajo: el `If-Match`, los conflictos con lo que cambió otro cliente, y qué
  hacer cuando dos dispositivos editaron al mismo tiempo.
- **Mostrar fotos.** Una `PHOTO` en base64 multiplica por diez el tamaño de la
  respuesta, y traerla para una lista donde no se ve sería gastar la conexión de
  la persona en nada.
- **Direcciones postales, cumpleaños y el resto de los campos.** Se leen el
  nombre, la organización, los correos, los teléfonos y las notas.

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
| `src-tauri/src/vcard.rs` | El parseo de las tarjetas. Las tres versiones del formato. |
| `src/tools/agenda.ts` | Ordenar y buscar. |

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
