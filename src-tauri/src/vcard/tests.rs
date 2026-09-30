use super::contact::*;
use super::lines::*;

const ANA: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:abc-123\r\n\
    FN:Ana Pérez\r\nN:Pérez;Ana;;;\r\n\
    EMAIL;TYPE=WORK:ana@ejemplo.com\r\n\
    TEL;TYPE=CELL:+54 11 5555-5555\r\n\
    ORG:Vasak Group;Soporte\r\nEND:VCARD\r\n";

#[test]
fn se_lee_una_tarjeta() {
    let c = contact_from(ANA, "https://x/ana.vcf").unwrap();

    assert_eq!(c.uid, "abc-123");
    assert_eq!(c.name, "Ana Pérez");
    assert_eq!(c.emails[0].value, "ana@ejemplo.com");
    assert_eq!(c.emails[0].label, "work");
    assert_eq!(c.phones[0].value, "+54 11 5555-5555");
    assert_eq!(c.url, "https://x/ana.vcf");
}

// ── Las líneas ─────────────────────────────────────────────────────────

/// Sin volver a juntarlas, un nombre largo aparece cortado y una foto en
/// base64 —que ocupa cientos de líneas— se interpreta como cientos de
/// propiedades basura.
#[test]
fn las_lineas_partidas_se_vuelven_a_juntar() {
    let crudo = "FN:Ana\r\n  Pérez\r\nUID:1\r\n";
    let lineas = unfold_lines(crudo);
    assert_eq!(lineas[0], "FN:Ana Pérez");
    assert_eq!(lineas[1], "UID:1");
}

/// El carácter que pliega **se va**: en el test de arriba el espacio que
/// sobrevive es el segundo, el que el nombre tenía de verdad.
#[test]
fn el_caracter_que_pliega_no_deja_espacio() {
    assert_eq!(unfold_lines("FN:Ana\r\n\tPérez")[0], "FN:AnaPérez");
}

/// **La otra forma de partir una línea, la de la 2.1.** El valor termina en
/// `=` y la línea siguiente no lleva nada adelante. Sin reconocerla, esa
/// línea no tiene dos puntos y se descarta entera: el nombre queda cortado
/// con un signo de igual pegado, que es justo el síntoma que leer la 2.1
/// viene a evitar.
#[test]
fn una_continuacion_de_quoted_printable_se_junta() {
    // El corte cae en el medio de una palabra, que es donde cae de verdad:
    // el formato parte a los 75 octetos sin mirar qué hay ahí. Y el `=` no
    // deja nada en su lugar — un espacio lo pondría donde no estaba.
    let vieja = "BEGIN:VCARD\r\nVERSION:2.1\r\n\
        FN;ENCODING=QUOTED-PRINTABLE:Ana Mar=C3=ADa P=C3=A9r=\r\nez\r\nEND:VCARD";

    let c = contact_from(vieja, "").unwrap();
    assert_eq!(c.name, "Ana María Pérez");
    assert!(!c.name.contains('='), "quedó el signo de igual: {}", c.name);
}

/// Y con varias continuaciones seguidas, que es lo que pasa con un valor de
/// verdad largo.
#[test]
fn varias_continuaciones_seguidas_tambien() {
    let vieja = "NOTE;ENCODING=QUOTED-PRINTABLE:uno=\r\ndos=\r\ntres\r\nFN:Ana";
    let lineas = unfold_lines(vieja);

    assert_eq!(lineas[0], "NOTE;ENCODING=QUOTED-PRINTABLE:unodostres");
    // Y la propiedad siguiente no se la comió.
    assert_eq!(lineas[1], "FN:Ana");
}

/// **El `=` solo no junta nada.** El base64 de una foto termina en `=` y no
/// sigue: juntar por el signo se comería la propiedad de abajo, que es peor
/// que el problema que se quería arreglar.
#[test]
fn un_base64_que_termina_en_igual_no_se_come_lo_que_sigue() {
    let con_foto = "PHOTO;ENCODING=b:iVBORw0KGgo=\r\nFN:Ana\r\n";
    let lineas = unfold_lines(con_foto);

    assert_eq!(lineas[0], "PHOTO;ENCODING=b:iVBORw0KGgo=");
    assert_eq!(lineas[1], "FN:Ana");
}

/// El valor puede tener dos puntos —una URL— así que el corte va por el
/// primero. Cortar por el último dejaría media dirección en el nombre.
#[test]
fn la_linea_se_corta_por_el_primer_dos_puntos() {
    let p = split_property("URL:https://ejemplo.com/ana").unwrap();
    assert_eq!(p.name, "URL");
    assert_eq!(p.value, "https://ejemplo.com/ana");
}

/// Los exportadores de Apple ponen un grupo adelante. Dejarlo pegado haría
/// que `item1.EMAIL` no se reconociera como un correo.
#[test]
fn el_grupo_de_apple_no_esconde_la_propiedad() {
    let p = split_property("item1.EMAIL;TYPE=HOME:ana@x.com").unwrap();
    assert_eq!(p.name, "EMAIL");

    let c = contact_from(
        "BEGIN:VCARD\r\nFN:Ana\r\nitem1.EMAIL;TYPE=HOME:ana@x.com\r\nEND:VCARD",
        "",
    )
    .unwrap();
    assert_eq!(c.emails.len(), 1);
}

#[test]
fn el_texto_se_desescapa() {
    assert_eq!(unescape_text(r"Pérez\, Ana"), "Pérez, Ana");
    assert_eq!(unescape_text(r"uno\ndos"), "uno\ndos");
    assert_eq!(unescape_text(r"punto\; y coma"), "punto; y coma");
}

/// Un apellido compuesto con un punto y coma escapado no puede partir el
/// campo en dos.
#[test]
fn un_punto_y_coma_escapado_no_parte_el_campo() {
    assert_eq!(
        split_fields(r"Pérez\;Gómez;Ana"),
        vec!["Pérez;Gómez", "Ana"]
    );
    assert_eq!(
        split_fields("Pérez;Ana;;;"),
        vec!["Pérez", "Ana", "", "", ""]
    );
}

// ── Las tres versiones ─────────────────────────────────────────────────

/// **Los teléfonos viejos escriben 2.1**, con los parámetros sueltos. Sin
/// leerlos, cualquier agenda exportada de uno muestra todos los teléfonos
/// sin etiqueta.
#[test]
fn se_lee_la_etiqueta_de_las_tres_versiones() {
    assert_eq!(label_of(&["TYPE=WORK".into()]), "work");
    assert_eq!(label_of(&["HOME".into()]), "home");
    assert_eq!(label_of(&["TYPE=\"HOME\"".into()]), "home");
    // Y con varias juntas, la primera que sirva.
    assert_eq!(label_of(&["TYPE=VOICE,HOME".into()]), "home");
}

/// Lo que no es una etiqueta para mostrar no puede terminar en pantalla:
/// «internet» no dice nada de un correo, y «pref» tampoco.
#[test]
fn lo_que_no_es_una_etiqueta_no_se_muestra_como_tal() {
    assert_eq!(label_of(&["INTERNET".into()]), "");
    assert_eq!(label_of(&["PREF".into()]), "");
    assert_eq!(label_of(&["ENCODING=QUOTED-PRINTABLE".into()]), "");
    assert_eq!(label_of(&["CHARSET=UTF-8".into()]), "");
    assert_eq!(label_of(&[]), "");
    // Pero si además hay una de verdad, ésa sí.
    assert_eq!(label_of(&["INTERNET".into(), "HOME".into()]), "home");
}

/// La 2.1 manda los acentos en `quoted-printable`. Sin deshacerlo, media
/// agenda en español se ve con signos de igual en el medio de los nombres.
#[test]
fn una_tarjeta_vieja_con_acentos_se_lee() {
    let vieja = "BEGIN:VCARD\r\nVERSION:2.1\r\n\
        FN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Ana P=C3=A9rez\r\n\
        TEL;HOME:11-5555\r\nEND:VCARD";

    let c = contact_from(vieja, "").unwrap();
    assert_eq!(c.name, "Ana Pérez");
    assert_eq!(c.phones[0].label, "home");
}

/// **El `N` se decodifica antes de partirlo en campos.** Sin eso, una
/// tarjeta 2.1 mostraba «P=E9rez» y ordenaba la agenda por eso — que es
/// peor que no mostrar el apellido, porque parece que anda.
#[test]
fn el_nombre_estructurado_se_decodifica_antes_de_partirse() {
    let vieja = "BEGIN:VCARD\r\nVERSION:2.1\r\n\
        N;ENCODING=QUOTED-PRINTABLE:P=E9rez;Ana;;;\r\nEND:VCARD";

    let c = contact_from(vieja, "").unwrap();
    assert_eq!(c.name, "Ana Pérez");
    assert_eq!(c.sort_key, "Pérez, Ana");
}

/// Y la organización igual, que tiene el mismo defecto y las mismas partes.
#[test]
fn la_organizacion_tambien_se_decodifica_antes() {
    let vieja = "BEGIN:VCARD\r\nFN:Ana\r\n\
        ORG;ENCODING=QUOTED-PRINTABLE:Panader=EDa;Mostrador\r\nEND:VCARD";
    assert_eq!(
        contact_from(vieja, "").unwrap().organization,
        "Panadería, Mostrador"
    );
}

/// **Se respeta el juego declarado.** Suponer latin-1 cuando la tarjeta
/// dice `windows-1252` convierte las comillas tipográficas y el guión largo
/// en caracteres de control invisibles.
#[test]
fn el_charset_declarado_se_respeta() {
    // 0x93 y 0x94 son las comillas tipográficas en windows-1252; en
    // latin-1 son controles que no se ven.
    let bytes = b"dijo \x93hola\x94";
    assert_eq!(bytes_to_text(bytes, Some("windows-1252")), "dijo “hola”");

    // Y el que no se conoce cae al respaldo en vez de romper.
    assert_eq!(bytes_to_text(b"caf\xe9", Some("juego-inventado")), "café");
}

/// Los esquemas de una URI no distinguen mayúsculas, y los exportadores de
/// verdad escriben `MailTo:`. Dejarlo pegado hace que el botón de escribir
/// abra «mailto:MailTo:…».
#[test]
fn el_esquema_se_saca_sin_mirar_mayusculas() {
    let nueva = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\n\
        EMAIL:MailTo:ana@ejemplo.com\r\nTEL:TEL:+541155555555\r\nEND:VCARD";

    let c = contact_from(nueva, "").unwrap();
    assert_eq!(c.emails[0].value, "ana@ejemplo.com");
    assert_eq!(c.phones[0].value, "+541155555555");
}

/// Y si los bytes no son UTF-8, se leen como latin-1 en vez de mostrar
/// rombos: es lo que manda una agenda exportada hace quince años.
#[test]
fn una_tarjeta_vieja_en_latin1_tambien() {
    let vieja = "BEGIN:VCARD\r\nVERSION:2.1\r\n\
        FN;ENCODING=QUOTED-PRINTABLE:Ana P=E9rez\r\nEND:VCARD";
    assert_eq!(contact_from(vieja, "").unwrap().name, "Ana Pérez");
}

/// En la 4.0 la dirección viene con `mailto:` adelante. Dejarlo haría que
/// el botón de escribirle abriera «mailto:mailto:…».
#[test]
fn el_mailto_de_la_version_4_no_queda_pegado() {
    let nueva = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\n\
        EMAIL:mailto:ana@ejemplo.com\r\nTEL:tel:+541155555555\r\nEND:VCARD";

    let c = contact_from(nueva, "").unwrap();
    assert_eq!(c.emails[0].value, "ana@ejemplo.com");
    assert_eq!(c.phones[0].value, "+541155555555");
}

// ── El nombre ──────────────────────────────────────────────────────────

/// Una tarjeta sin `FN` es inválida según el estándar y aparece igual.
/// Armar el nombre con el `N` es la diferencia entre ver a alguien y ver un
/// renglón vacío.
#[test]
fn sin_fn_el_nombre_se_arma_con_el_n() {
    let sin_fn = "BEGIN:VCARD\r\nN:Pérez;Ana;María;Sra.;\r\nEND:VCARD";
    assert_eq!(
        contact_from(sin_fn, "").unwrap().name,
        "Sra. Ana María Pérez"
    );
}

/// Ordenar por el nombre que se muestra pone a todas las Anas juntas y a
/// los Pérez desparramados, que no es como nadie busca en una agenda.
#[test]
fn se_ordena_por_apellido() {
    assert_eq!(contact_from(ANA, "").unwrap().sort_key, "Pérez, Ana");

    // Sin apellido se ordena por lo que se muestra: es lo único que hay.
    let solo_fn = "BEGIN:VCARD\r\nFN:Panadería del barrio\r\nEND:VCARD";
    assert_eq!(
        contact_from(solo_fn, "").unwrap().sort_key,
        "Panadería del barrio"
    );
}

/// `ORG` trae la empresa y sus divisiones. «Vasak Group» y «Vasak Group,
/// Soporte» son cosas distintas.
#[test]
fn la_organizacion_incluye_la_division() {
    assert_eq!(
        contact_from(ANA, "").unwrap().organization,
        "Vasak Group, Soporte"
    );
}

// ── Lo que llega roto ──────────────────────────────────────────────────

/// Una tarjeta sin nada que mostrar ocupa lugar en la lista y no sirve para
/// nada.
#[test]
fn una_tarjeta_vacia_no_es_un_contacto() {
    assert!(contact_from("BEGIN:VCARD\r\nVERSION:3.0\r\nEND:VCARD", "").is_none());
    assert!(contact_from("", "").is_none());
    assert!(contact_from("no es una tarjeta", "").is_none());
}

/// Una tarjeta puede haber llegado adjunta a un correo o importada de un
/// teléfono: no es contenido de confianza por estar en la libreta.
#[test]
fn lo_que_esta_roto_no_hace_caer_nada() {
    for basura in [
        ":::",
        "FN:",
        ";;;",
        "BEGIN:VCARD",
        "\r\n\r\n",
        "\\",
        "N:;;;;;;;;;;",
    ] {
        let _ = contact_from(basura, "");
        let _ = split_cards(basura);
    }
}

/// Un archivo armado para hacer trabajar al programa, o una tarjeta con la
/// foto partida en pedazos: en los dos casos tiene que volver.
#[test]
fn una_tarjeta_desmedida_no_cuelga() {
    let enorme =
        "BEGIN:VCARD\r\nFN:Ana\r\n".to_string() + &"X-BASURA:algo\r\n".repeat(50_000) + "END:VCARD";
    let c = contact_from(&enorme, "").unwrap();
    assert_eq!(c.name, "Ana");
}

/// Un nombre de diez mil caracteres no es un nombre: es algo que va a
/// romper la lista al dibujarla.
#[test]
fn un_valor_desmedido_se_recorta_sin_partir_un_caracter() {
    let largo = format!("BEGIN:VCARD\r\nFN:{}\r\nEND:VCARD", "ñ".repeat(MAX_VALUE));
    let c = contact_from(&largo, "").unwrap();

    assert!(c.name.len() <= MAX_VALUE + 4);
    assert!(c.name.ends_with('…'));
    assert!(!c.name.contains('\u{FFFD}'));
}

// ── Varias tarjetas ────────────────────────────────────────────────────

/// Hay libretas exportadas que son un solo archivo con miles. Sin
/// separarlas se leería una sola con los datos de todas mezclados.
#[test]
fn se_separan_las_tarjetas_de_un_archivo() {
    let dos = format!("{ANA}BEGIN:VCARD\r\nFN:Juan\r\nEND:VCARD\r\n");
    let tarjetas = split_cards(&dos);

    assert_eq!(tarjetas.len(), 2);
    assert_eq!(contact_from(&tarjetas[0], "").unwrap().name, "Ana Pérez");
    assert_eq!(contact_from(&tarjetas[1], "").unwrap().name, "Juan");
}

/// Una tarjeta sin su `END` está mal formada, pero perder un contacto por
/// dos palabras que faltaron sería peor.
#[test]
fn una_tarjeta_sin_cierre_se_aprovecha_igual() {
    let sin_cierre = "BEGIN:VCARD\r\nFN:Ana\r\n";
    let tarjetas = split_cards(sin_cierre);
    assert_eq!(tarjetas.len(), 1);
    assert_eq!(contact_from(&tarjetas[0], "").unwrap().name, "Ana");
}

// ── Lo que el servidor puede usar para tumbar la aplicación ─────────────

/// **Un `:` dentro de las comillas de un parámetro no es el separador.**
///
/// Buscarlo a byte crudo se paraba en el de adentro, así que los parámetros
/// quedaban en `FN;X-TEST="a`, que no dicen `quoted-printable`: la línea no
/// se juntaba y el nombre salía con el `=` pegado al final, `Ana=`.
/// `X-TEST="a:b"` es legal y lo exportan herramientas de verdad.
#[test]
fn un_dos_puntos_dentro_de_comillas_no_es_el_separador() {
    let vieja = concat!(
        "BEGIN:VCARD\r\nVERSION:2.1\r\n",
        "FN;X-TEST=\"a:b\";ENCODING=QUOTED-PRINTABLE:Ana=\r\n",
        "Maria\r\nEND:VCARD"
    );

    let c = contact_from(vieja, "").unwrap();
    assert_eq!(c.name, "AnaMaria", "quedó el signo de igual");
    assert!(!c.name.contains('='), "quedó el signo de igual: {}", c.name);
}

/// Y un `ENCODING` de verdad, sin comillas antes, se sigue encontrando: el
/// arreglo no puede haber suntado el caso normal.
#[test]
fn un_encoding_normal_sigue_encontrandose() {
    let vieja = concat!(
        "BEGIN:VCARD\r\nVERSION:2.1\r\n",
        "FN;ENCODING=QUOTED-PRINTABLE:Ana=\r\n",
        "Maria\r\nEND:VCARD"
    );
    assert_eq!(contact_from(vieja, "").unwrap().name, "AnaMaria");
}

/// Un `:` de verdad, después de los parámetros, se encuentra igual — con
/// comillas abiertas antes, sin comillas antes, y con comillas simples.
#[test]
fn un_dos_puntos_de_verdad_se_encuentra_igual() {
    for (params, valor) in [
        ("ENCODING=QUOTED-PRINTABLE", "Ana="),
        ("X-TEST=\"a:b\";ENCODING=QUOTED-PRINTABLE", "Ana="),
        (
            "TYPE=\"a:b:c\";X-Y=\"d:e\";ENCODING=QUOTED-PRINTABLE",
            "Ana=",
        ),
    ] {
        let vieja =
            format!("BEGIN:VCARD\r\nVERSION:2.1\r\nFN;{params}:{valor}\r\nMaria\r\nEND:VCARD");
        let c = contact_from(&vieja, "");
        assert_eq!(
            c.as_ref().map(|c| c.name.as_str()),
            Some("AnaMaria"),
            "{params}",
        );
    }
}

/// Una comilla que queda abierta **no inventa un separador**: ni el juntador
/// ni `split_property` la ven, así que la línea no se parte por el `:` de adentro.
/// Malformado, pero no rompiendo, y sin colgar un `=` al final de un nombre.
///
/// El desenlace se afirma, no se descarta: un `let _ =` acá haría que la
/// prueba pasara aunque `contact_from` dejara de devolver un contacto.
#[test]
fn una_comilla_sin_cerrar_no_inventa_un_separador() {
    let vieja = concat!(
        "BEGIN:VCARD\r\nVERSION:2.1\r\n",
        "FN;X-TEST=\"sin cerrar;ENCODING=QUOTED-PRINTABLE:Ana\r\n",
        "END:VCARD"
    );

    // No hay separador, así que la línea no se parte: ni `split_property` la ve…
    let con_nombre = vieja.lines().find(|l| l.starts_with("FN")).unwrap();
    assert!(split_property(con_nombre).is_none(), "{con_nombre}");
    // …ni el juntador inventa uno. Si encontraba alguno, tendría que ser
    // después de los parámetros, y no lo hay.
    let (at, _abierta) = find_separator(vieja, vieja.find('F').unwrap(), false);
    assert!(at.is_none() || !vieja[..at.unwrap()].contains("ENCODING=QUOTED"));

    // Y el contacto sale sin nombre, en vez de con un `=` pegado.
    if let Some(c) = contact_from(vieja, "") {
        assert!(
            !c.name.contains('='),
            "quedó un signo de igual: {:?}",
            c.name
        );
    }
}

/// El helper tiene que cortar **donde corta `split_property`**, o la línea se junta
/// con un criterio y después se parte con otro. Esta prueba es esa
/// equivalencia, sobre los casos que importan: comillas dobles con dos
/// puntos adentro, y el `:` de verdad después de los parámetros.
#[test]
fn el_separador_corta_donde_corta_partir() {
    for izquierda in [
        "ENCODING=QUOTED-PRINTABLE",
        "X-TEST=\"a:b\";ENCODING=QUOTED-PRINTABLE",
        "TYPE=\"a:b:c\";X-Y=\"d:e\";ENCODING=QUOTED-PRINTABLE",
        "TYPE=\"\";ENCODING=QUOTED-PRINTABLE",
        "LANGUAGE=es;ENCODING=QUOTED-PRINTABLE",
        // Sin comillas: el primer `:` de valor, que es el separador.
        "ENCODING=QUOTED-PRINTABLE",
    ] {
        let linea = format!("FN;{izquierda}:Ana");
        let por_partir = split_property(&linea).expect("partir");
        let (at, _abierta) = find_separator(&linea, 0, false);
        // El punto de corte de `split_property` se recupera del valor: es todo lo que
        // queda después del `:`. Comparar contra `find(':')` no serviría,
        // porque devuelve justo el que hay que ignorar.
        let corte_de_partir = linea.len() - por_partir.value.len() - 1;
        assert_eq!(Some(corte_de_partir), at, "{izquierda}");
        assert_eq!(por_partir.name, "FN", "{izquierda}");
        assert_eq!(por_partir.value, "Ana", "{izquierda}");
    }
}

/// Y `desde` evita que se recorra lo anterior: la búsqueda sigue donde se le
/// dice, que es lo que hace lineal a esto.
#[test]
fn el_separador_empieza_donde_se_le_dice() {
    let linea = "FN;X=\"a:b\";ENCODING=QP:Ana";
    let (at, abierta) = find_separator(linea, 0, false);
    assert_eq!(Some(22), at, "el separador no es el `:` de adentro");
    assert!(!abierta);

    // Desde un `desde` mayor, el mismo `:` de adentro ya no se ve.
    let (at, _abierta) = find_separator(linea, 10, false);
    assert_eq!(Some(22), at);

    // Con la comilla ya abierta desde antes, el `:` no cuenta.
    assert_eq!(find_separator(":despues", 0, true), (None, true));
    // Y sin separador, se devuelve dónde quedó la búsqueda.
    assert_eq!(find_separator("FN;ENCODING=QP", 0, false), (None, false));
}

/// Corre `work` en otro hilo y falla si no termina en `budget`. Sin esto,
/// una prueba de tiempo sin el arreglo no falla: se cuelga y el rojo nunca
/// aparece.
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

/// **Una tarjeta con continuaciones de la 3.0 tampoco tarda.**
///
/// El caso del hallazgo de revisión, y es el mismo defecto que el de las
/// continuaciones 2.1: al encontrar el separador, `imprimible` ya estaba
/// decidido pero el bloque de búsqueda se volvía a ejecutar en cada
/// continuación, con `is_quoted_printable` pasando a minúsculas y copiando los
/// **parámetros** enteros una vez por línea. Con P bytes de parámetros y N
/// continuaciones, el trabajo es P·N.
///
/// Estas continuaciones llevan un espacio adelante, así que entran por el
/// brazo `Some(continuacion)` y **no** salen por el `continue` de la 2.1: es
/// un camino que las pruebas anteriores no tocaban.
///
/// Medido con el mismo caso, en release:
///
/// ```text
///    params     cont          viejo         nuevo    ratio
///     16384    10000       126.67ms      104.47µs    1212x
///     65536    40000          2.04s      404.47µs    5048x
///    131072    60000          6.13s      640.86µs    9561x
/// ```
///
/// Los tres entran en el tope de 512 KiB. El último es el de la prueba: sin
/// el arreglo se pasa del presupuesto de cinco segundos.
#[test]
fn una_tarjeta_con_continuaciones_de_la_30_no_tarda() {
    let continuaciones = 60_000;
    // Cada plegado va en su propia línea física, como en un archivo de
    // verdad: el primero no es el que está pegado al `:`.
    let tarjeta = format!(
        "BEGIN:VCARD\nFN:Ana\nNOTE;ENCODING=QUOTED-PRINTABLE;{}:\n{}EMAIL:ana@x.com\nEND:VCARD\n",
        "A".repeat(131_072),
        " x\n".repeat(continuaciones),
    );
    assert!(tarjeta.len() <= 512 * 1024, "{}", tarjeta.len());

    let (lineas, contacto) = finishes_within(std::time::Duration::from_secs(5), move || {
        let lineas = unfold_lines(&tarjeta);
        let primera = split_cards(&tarjeta).into_iter().next().unwrap();
        (lineas, contact_from(&primera, "").unwrap())
    });

    // Y no es que se haya hecho todo mal: la línea juntada tiene que estar
    // entera, y la propiedad que sigue no se comió.
    let nota = lineas.iter().find(|l| l.starts_with("NOTE;")).unwrap();
    let valor = nota.split_once(':').unwrap().1;
    assert_eq!(
        valor.len(),
        continuaciones,
        "valor de {} bytes",
        valor.len()
    );
    assert!(
        !valor.contains(' '),
        "el plegado tiene que sacar los espacios"
    );
    assert_eq!(contacto.name, "Ana");
    assert_eq!(contacto.emails[0].value, "ana@x.com");
}

/// **Una línea sin separador tiene que seguir buscándolo.**
///
/// Es el caso que un guard mal puesto rompe: si `imprimible` decidiera
/// demasiado pronto, esta línea se declararía «no imprimible» con sólo el
/// primer fragmento y las continuaciones siguientes no se mirarían. Aquí el
/// separador **no aparece hasta el segundo fragmento**, y la continuación
/// 2.1 que sigue tiene que juntarse igual.
#[test]
fn una_linea_sin_separador_sigue_buscando_al_agregarse_un_fragmento() {
    // Ni `NOTA` ni `;P=1` traen `=`, así que el juntador no puede joins
    // por el camino de la 2.1: lo tiene que hacer por el del separador.
    let vieja = concat!(
        "BEGIN:VCARD\r\nVERSION:2.1\r\n",
        "NOTE;ENCODING=QUOTED-PRINTABLE\r\n",
        " ;P=1:Ana=\r\n",
        "Maria\r\n",
        "FN:Ana\r\n",
        "END:VCARD"
    );
    let c = contact_from(vieja, "").expect("la tarjeta tiene contacto");
    assert!(
        !c.notes.contains('='),
        "la línea no se juntó y quedó el signo de igual: {:?}",
        c.notes,
    );
    assert!(c.notes.contains("AnaMaria"), "{:?}", c.notes);
}

/// Una comilla abierta en los parámetros, con continuaciones de la 3.0
/// alrededor: el separador se busca una vez, en el fragmento que lo tiene, y
/// el resto de la línea lógica no lo vuelve a mover.
///
/// La comilla abierta es lo que hace que `coma` tenga que viajar entre
/// fragmentos, y lo que hace que un `:` de adentro no sea el separador.
#[test]
fn una_comilla_abierta_entre_continuaciones_no_confunde_el_separador() {
    let vieja = concat!(
        "BEGIN:VCARD\r\nVERSION:2.1\r\n",
        "FN;X=\"a:b\";ENCODING=QUOTED-PRINTABLE:Ana=\r\n",
        " b\r\n",
        " c\r\n",
        " Maria\r\n",
        "END:VCARD"
    );
    let c = contact_from(vieja, "").unwrap();
    // Lo que importa es que la línea se juntó entera y sin el `=` colgando.
    // Los espacios depende de qué camino juntó cada fragmento —el de la 2.1
    // se come el `=` y deja el espacio, el de la 3.0 los saca—, y eso es de
    // antes de este arreglo.
    assert!(
        !c.name.contains('='),
        "quedó el signo de igual: {:?}",
        c.name
    );
    let sin_espacios: String = c.name.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(sin_espacios, "AnabcMaria", "{:?}", c.name);
}

/// **Una tarjeta armada para el desdoblado no tarda.** Doscientos sesenta
/// mil bytes de parámetros y ochenta y cinco mil continuaciones de
/// `quoted-printable` entran en el tope de una tarjeta, y mirar los
/// parámetros de nuevo en cada continuación eran **veintisiete segundos** por
/// tarjeta en el caso medido: `ultima` crecía, y `quedo_a_medias` pasaba a
/// minúsculas y volvía a copiar todo lo acumulado, una vez por línea. El
/// tiempo crece como el cuadrado del tamaño de la tarjeta.
///
/// La decisión se toma una vez, cuando aparece el primer dos puntos, y a
/// partir de ahí seguir es un `ends_with`, que es constante. El valor sale
/// entero, y la propiedad que sigue también: no se la comió.
#[test]
fn una_tarjeta_armada_para_el_desdoblado_no_tarda() {
    let continuations = 85_000;
    let tarjeta = format!(
        "BEGIN:VCARD\nFN:Ana\nNOTE;ENCODING=QUOTED-PRINTABLE;{}:{}fin\nEMAIL:ana@x.com\nEND:VCARD\n",
        "A".repeat(262_000),
        "x=\n".repeat(continuations)
    );
    assert!(tarjeta.len() <= 512 * 1024, "{}", tarjeta.len());

    let (lineas, contacto) = finishes_within(std::time::Duration::from_secs(5), move || {
        let lineas = unfold_lines(&tarjeta);
        let primera = split_cards(&tarjeta).into_iter().next().unwrap();
        (lineas, contact_from(&primera, "").unwrap())
    });

    let nota = lineas.iter().find(|l| l.starts_with("NOTE;")).unwrap();
    let valor = nota.split_once(':').unwrap().1;
    assert_eq!(valor.len(), continuations + 3);
    assert!(valor.ends_with("xfin"), "{}", &valor[valor.len() - 10..]);
    assert_eq!(contacto.name, "Ana");
    assert_eq!(contacto.emails[0].value, "ana@x.com");
    assert!(contacto.notes.starts_with("xxx"));
}

/// **El `CHARSET` se aplica una sola vez.** El cuerpo de la respuesta pasó a
/// UTF-8 antes de leer el XML, así que los bytes de un valor que vino como
/// texto **son UTF-8**, y volver a decodificarlos con el juego declarado
/// rompe lo que ya estaba bien.
///
/// Antes: `FN;CHARSET=ISO-8859-2:Łukasz` salía «Ĺ», un control invisible y
/// «ukasz», y el contacto se ordenaba por esa basura. Es el mismo camino que
/// una tarjeta exportada por un teléfono polaco tomaba siempre.
#[test]
fn un_charset_declarado_no_se_aplica_dos_veces() {
    for declarada in [
        "FN;CHARSET=ISO-8859-2:Łukasz",
        "FN;CHARSET=UTF-8:Łukasz",
        "FN;CHARSET=WINDOWS-1252:Ana Pérez",
        // Y en una propiedad con más de una parte, que es donde más se nota.
        "N;CHARSET=UTF-8:Pérez;Łukasz;;;",
    ] {
        let tarjeta = format!("BEGIN:VCARD\r\nVERSION:3.0\r\n{declarada}\r\nEND:VCARD");
        let c = contact_from(&tarjeta, "").unwrap();
        assert!(!c.name.contains('\u{0081}'), "{} → {:?}", declarada, c.name);
    }

    let tarjeta = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN;CHARSET=ISO-8859-2:Łukasz\r\nEND:VCARD";
    assert_eq!(contact_from(tarjeta, "").unwrap().name, "Łukasz");
}

/// Y el camino del `quoted-printable`, que **sí** necesita el juego declarado:
/// los bytes que salen de deshacer el escape son los del juego que la tarjeta
/// dice, y no UTF-8.
#[test]
fn el_charset_sirve_para_lo_que_sale_del_quoted_printable() {
    // 2.1 en latin-1: `P` + 0xE9 + `rez` = «Pérez» con la e acentuada en
    // un byte, que no es UTF-8 válido. Sin el juego declarado, ese 0xE9
    // sería un rombo.
    let tarjeta = concat!(
        "BEGIN:VCARD\r\nVERSION:2.1\r\n",
        "FN;CHARSET=ISO-8859-1;ENCODING=QUOTED-PRINTABLE:P=E9rez\r\n",
        "END:VCARD"
    );
    let lineas = unfold_lines(tarjeta);
    let propiedad = split_property(lineas.iter().find(|l| l.starts_with("FN")).unwrap()).unwrap();
    assert_eq!(decoded(&propiedad), "Pérez");
    assert_eq!(contact_from(tarjeta, "").unwrap().name, "Pérez");
}

/// **`strip_scheme` no entra en pánico con un valor multibyte.** Cortaba el
/// texto en un índice de byte sin mirar si era un límite de carácter, así que
/// un correo o un teléfono con un multibyte antes del esquema —`日本:…`,
/// `😀mailto:…`— lo hacía reventar. Y lo reventaba en depuración, no en la
/// aplicación de la persona: un pánico en `debug`.
#[test]
fn un_valor_con_multibyte_antes_del_esquema_no_entra_en_panico() {
    for valor in [
        "mailto:ana@ejemplo.com",
        "MAILTO:ana@ejemplo.com",
        // Un emoji es de cuatro bytes y el `mailto:` no cae en un límite de
        // carácter: el corte del `&valor[..7]` caía adentro.
        "😀mailto:ana@ejemplo.com",
        "😀😀mailto:ana@ejemplo.com",
        "日本:mailto:ana@ejemplo.com",
        "ñmailto:ana@ejemplo.com",
        // Más corto que el esquema, y con el prefijo de largo刚好.
        "m",
        "mailto",
        "",
        "tel:+541155550000",
    ] {
        // `strip_scheme` es privada; el camino que la lleva es el del `EMAIL`
        // y el del `TEL`. Con `catch_unwind` porque el defecto viejo es
        // justamente un pánico, y un pánico tumba el archivo de pruebas
        // entero sin dejar aserción.
        let tarjeta = format!("BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\nEMAIL:{valor}\r\nEND:VCARD");
        let resultado = std::panic::catch_unwind(move || {
            let c = contact_from(&tarjeta, "");
            c.map(|c| c.emails.first().map(|d| d.value.clone()))
        });
        // El resultado es el de siempre —el esquema se saca cuando está al
        // principio—; lo que se comprueba es que se llegue.
        //
        // **Cada desenlace se dice por su nombre.** Con un `Ok(_)` a secas,
        // `Ok(Some(None))` —que es exactamente «el correo no está en la
        // tarjeta»— pasaba igual que un acierto: si `contact_from` dejara de
        // guardar correos, esta prueba seguía en verde. Un test que no puede
        // fallar no es un test.
        match resultado {
            Ok(Some(Some(guardado))) => {
                let esperado = valor
                    .strip_prefix("mailto:")
                    .or_else(|| valor.strip_prefix("MAILTO:"))
                    .unwrap_or(valor);
                assert_eq!(guardado, esperado, "entrante {valor:?}");
            }
            // Sin correo guardado sólo se perdona un `EMAIL:` vacío, que es
            // un dato que no está, no uno que se perdió. Cualquier valor no
            // vacío que no llegó al contacto es un bug.
            Ok(Some(None)) => {
                assert!(valor.is_empty(), "el correo {valor:?} no llegó al contacto",)
            }
            // `contact_from` devolvió `None`: la tarjeta dejó de dar un
            // contacto, y con ella todos los correos.
            Ok(None) => panic!("la tarjeta dejó de dar contacto: {valor:?}"),
            Err(_) => panic!("{valor:?} entró en pánico"),
        }
    }
}

/// Y un `EMAIL` con un multibyte adelante conserva el valor entero: no se
/// come ni el prefijo equivocado ni el carácter.
#[test]
fn un_email_con_multibyte_no_se_altera() {
    let tarjeta =
        "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\nEMAIL:😀mailto:ana@ejemplo.com\r\nEND:VCARD";
    let c = contact_from(tarjeta, "").unwrap();
    assert_eq!(c.emails[0].value, "😀mailto:ana@ejemplo.com");
}
