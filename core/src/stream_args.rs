//! Construction pure des arguments FFmpeg pour la diffusion en direct.
//! Aucune dépendance : testable en isolation (`cargo test -p lightbrowser-core`).
//! v2 : micro + son système (périphériques dshow), mixage, volumes, liste des périphériques.

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Screen,
    Window(String),
    Webcam(String),
    Region {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    Pause(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Webcam incrustée sur la source principale (Screen ou Window).
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    /// Nom dshow de la webcam incrustée.
    pub webcam: String,
    pub corner: Corner,
    /// Largeur de l'incrustation en % de la largeur de sortie (5-50).
    pub size_pct: u32,
    /// Marge en pixels depuis le bord (0-200).
    pub margin_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoder {
    Nvenc,
    Qsv,
    Amf,
    X264,
}

impl Encoder {
    pub fn name(self) -> &'static str {
        match self {
            Encoder::Nvenc => "h264_nvenc",
            Encoder::Qsv => "h264_qsv",
            Encoder::Amf => "h264_amf",
            Encoder::X264 => "libx264",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub source: Source,
    /// Microphone (nom dshow). None = pas de micro.
    pub audio_device: Option<String>,
    /// Son du système (nom dshow : « Stereo Mix », « virtual-audio-capturer », câble virtuel…).
    pub system_audio_device: Option<String>,
    /// Gains 0.0 à 2.0 (1.0 = inchangé).
    pub mic_volume: f32,
    pub system_volume: f32,
    /// Incrustation webcam (None = aucune).
    pub overlay: Option<Overlay>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub encoder: Encoder,
    /// URLs RTMP(S) complètes, clé de flux incluse.
    pub outputs: Vec<String>,
    /// Enregistrement local MKV (chemin avec « / », sans | " ' \ [ ]). None = pas d'enregistrement.
    pub record_path: Option<String>,
    /// Capture DXGI via ddagrab pour Source::Screen (ignoré pour Window et Webcam).
    pub use_ddagrab: bool,
}

impl Config {
    /// Profil par défaut : 1080p30, 6000 kbps CBR, repli x264, silence si aucune source audio.
    pub fn new(source: Source, outputs: Vec<String>) -> Self {
        Self {
            source,
            audio_device: None,
            system_audio_device: None,
            mic_volume: 1.0,
            system_volume: 1.0,
            overlay: None,
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate_kbps: 6000,
            encoder: Encoder::X264,
            outputs,
            record_path: None,
            use_ddagrab: false,
        }
    }
}

/// Encodage d'essai d'une image : réussit seulement si l'encodeur est réellement utilisable.
pub fn probe_args(enc: Encoder) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=black:s=256x256:r=1",
        "-frames:v",
        "1",
        "-c:v",
        enc.name(),
        "-f",
        "null",
        "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Arguments pour lister les périphériques DirectShow (résultat sur stderr, code retour 1 normal).
pub fn list_devices_args() -> Vec<String> {
    [
        "-hide_banner",
        "-list_devices",
        "true",
        "-f",
        "dshow",
        "-i",
        "dummy",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Devices {
    pub video: Vec<String>,
    pub audio: Vec<String>,
}

/// Analyse la sortie de `ffmpeg -list_devices` (ancien format par sections et nouveau format « (video) / (audio) »).
pub fn parse_dshow_devices(out: &str) -> Devices {
    let mut d = Devices::default();
    let mut section = 0u8; // 1 = vidéo, 2 = audio
    for line in out.lines() {
        let l = line.trim();
        if l.contains("DirectShow video devices") {
            section = 1;
            continue;
        }
        if l.contains("DirectShow audio devices") {
            section = 2;
            continue;
        }
        if l.contains("Alternative name") {
            continue;
        }
        let Some(s) = l.find('"') else { continue };
        let rest = &l[s + 1..];
        let Some(e) = rest.find('"') else { continue };
        let name = rest[..e].trim().to_string();
        if name.is_empty() || name.starts_with('@') {
            continue;
        }
        let tail = rest[e + 1..].to_ascii_lowercase();
        if tail.contains("none") {
            continue;
        }
        let (mut v, mut au) = (tail.contains("video"), tail.contains("audio"));
        if !v && !au {
            v = section == 1;
            au = section == 2;
        }
        if v && !d.video.contains(&name) {
            d.video.push(name.clone());
        }
        if au && !d.audio.contains(&name) {
            d.audio.push(name);
        }
    }
    d
}

/// Encodage d'essai d'une image avec ddagrab : réussit seulement si la capture DXGI fonctionne vraiment.
pub fn probe_ddagrab_args() -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "ddagrab=framerate=1",
        "-frames:v",
        "1",
        "-vf",
        "hwdownload,format=bgra",
        "-f",
        "null",
        "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Graphe vidéo : source principale mise à l'échelle, webcam (entrée 1) réduite puis incrustée dans un coin.
pub fn overlay_graph(w: u32, h: u32, o: &Overlay) -> String {
    overlay_graph_pre(w, h, o, "")
}

pub fn overlay_graph_pre(w: u32, h: u32, o: &Overlay, pre: &str) -> String {
    let ow = (w * o.size_pct / 100) & !1; // largeur paire
    let m = o.margin_px;
    let (x, y) = match o.corner {
        Corner::TopLeft => (format!("{m}"), format!("{m}")),
        Corner::TopRight => (format!("main_w-overlay_w-{m}"), format!("{m}")),
        Corner::BottomLeft => (format!("{m}"), format!("main_h-overlay_h-{m}")),
        Corner::BottomRight => (
            format!("main_w-overlay_w-{m}"),
            format!("main_h-overlay_h-{m}"),
        ),
    };
    format!(
        "[0:v]{pre}scale={w}:{h}:force_original_aspect_ratio=decrease:flags=fast_bilinear,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2[base];[1:v]scale={ow}:-2:flags=fast_bilinear[cam];[base][cam]overlay={x}:{y},format=yuv420p[v]"
    )
}

fn validate(c: &Config) -> Result<(), String> {
    if c.outputs.is_empty() {
        return Err("aucune destination".into());
    }
    if !(1..=60).contains(&c.fps) {
        return Err("fps hors limites (1-60)".into());
    }
    if !(500..=20000).contains(&c.bitrate_kbps) {
        return Err("débit hors limites (500-20000 kbps)".into());
    }
    if c.width == 0 || c.height == 0 || c.width % 2 != 0 || c.height % 2 != 0 {
        return Err("résolution invalide (dimensions paires requises)".into());
    }
    for o in &c.outputs {
        if !(o.starts_with("rtmp://") || o.starts_with("rtmps://")) {
            return Err("destination : URL rtmp(s):// requise".into());
        }
        if o.chars()
            .any(|ch| matches!(ch, '|' | '"' | ' ' | '[' | ']'))
        {
            return Err("destination : caractères interdits dans l'URL".into());
        }
    }
    let empty = |s: &String| s.trim().is_empty();
    match &c.source {
        Source::Window(t) if empty(t) => return Err("titre de fenêtre vide".into()),
        Source::Webcam(n) if empty(n) => return Err("nom de webcam vide".into()),
        Source::Region { width, height, .. } => {
            if *width == 0 || *height == 0 || *width % 2 != 0 || *height % 2 != 0 {
                return Err("région : dimensions paires requises".into());
            }
        }
        _ => {}
    }
    if matches!(&c.audio_device, Some(d) if empty(d)) {
        return Err("nom du micro vide".into());
    }
    if matches!(&c.system_audio_device, Some(d) if empty(d)) {
        return Err("nom du périphérique de son système vide".into());
    }
    if c.audio_device.is_some() && c.audio_device == c.system_audio_device {
        return Err("micro et son système : même périphérique".into());
    }
    if !(0.0..=2.0).contains(&c.mic_volume) || !(0.0..=2.0).contains(&c.system_volume) {
        return Err("volume hors limites (0.0-2.0)".into());
    }
    if let Some(o) = &c.overlay {
        if matches!(c.source, Source::Webcam(_)) {
            return Err("incrustation : la source principale ne peut pas être la webcam".into());
        }
        if o.webcam.trim().is_empty() {
            return Err("incrustation : nom de webcam vide".into());
        }
        if !(5..=50).contains(&o.size_pct) {
            return Err("incrustation : taille hors limites (5-50 %)".into());
        }
        if o.margin_px > 200 {
            return Err("incrustation : marge hors limites (0-200 px)".into());
        }
    }
    if let Some(r) = &c.record_path {
        if r.trim().is_empty() {
            return Err("enregistrement : chemin vide".into());
        }
        if !r.to_ascii_lowercase().ends_with(".mkv") {
            return Err("enregistrement : extension .mkv requise".into());
        }
        if r.chars()
            .any(|ch| matches!(ch, '|' | '"' | '\'' | '\\' | '[' | ']'))
        {
            return Err("enregistrement : caractères interdits dans le chemin".into());
        }
    }
    Ok(())
}

fn p(a: &mut Vec<String>, xs: &[&str]) {
    a.extend(xs.iter().map(|s| s.to_string()));
}

pub fn build_args(c: &Config) -> Result<Vec<String>, String> {
    validate(c)?;
    let fps = c.fps.to_string();
    let b = c.bitrate_kbps;
    let (bitrate, bufsize, gop) = (
        format!("{b}k"),
        format!("{}k", b * 2),
        (c.fps * 2).to_string(),
    );
    let mut a: Vec<String> = Vec::new();
    p(
        &mut a,
        &["-hide_banner", "-loglevel", "warning", "-nostats", "-y"],
    );

    let use_dda = (matches!(c.source, Source::Screen) || matches!(c.source, Source::Region { .. }))
        && c.use_ddagrab;
    let pre = if use_dda {
        "hwdownload,format=bgra,"
    } else {
        ""
    };

    // Entrée 0 : vidéo
    match &c.source {
        Source::Screen => {
            if use_dda {
                let i = format!("ddagrab=framerate={fps}:draw_mouse=1");
                p(&mut a, &["-f", "lavfi", "-i", &i]);
            } else {
                p(
                    &mut a,
                    &[
                        "-thread_queue_size",
                        "512",
                        "-f",
                        "gdigrab",
                        "-framerate",
                        &fps,
                        "-i",
                        "desktop",
                    ],
                );
            }
        }
        Source::Region {
            x,
            y,
            width,
            height,
        } => {
            if use_dda {
                let i = format!("ddagrab=framerate={fps}:draw_mouse=1:offset_x={x}:offset_y={y}:video_size={width}x{height}");
                p(&mut a, &["-f", "lavfi", "-i", &i]);
            } else {
                p(
                    &mut a,
                    &[
                        "-thread_queue_size",
                        "512",
                        "-f",
                        "gdigrab",
                        "-framerate",
                        &fps,
                        "-offset_x",
                        &x.to_string(),
                        "-offset_y",
                        &y.to_string(),
                        "-video_size",
                        &format!("{width}x{height}"),
                        "-i",
                        "desktop",
                    ],
                );
            }
        }
        Source::Pause(path) => {
            if !path.is_empty() && std::path::Path::new(path).exists() {
                p(&mut a, &["-loop", "1", "-framerate", &fps, "-i", path]);
            } else {
                let i = format!("color=c=#14151a:s={}x{}:r={fps}", c.width, c.height);
                p(&mut a, &["-f", "lavfi", "-i", &i]);
            }
        }
        Source::Window(t) => {
            let i = format!("title={t}");
            p(
                &mut a,
                &[
                    "-thread_queue_size",
                    "512",
                    "-f",
                    "gdigrab",
                    "-framerate",
                    &fps,
                    "-i",
                    &i,
                ],
            );
        }
        Source::Webcam(n) => {
            let i = format!("video={n}");
            p(
                &mut a,
                &[
                    "-thread_queue_size",
                    "512",
                    "-f",
                    "dshow",
                    "-framerate",
                    &fps,
                    "-i",
                    &i,
                ],
            );
        }
    }

    // Entrée 1 (optionnelle) : webcam incrustée ; les entrées audio suivent.
    if let Some(o) = &c.overlay {
        let i = format!("video={}", o.webcam);
        p(
            &mut a,
            &[
                "-thread_queue_size",
                "512",
                "-f",
                "dshow",
                "-framerate",
                &fps,
                "-i",
                &i,
            ],
        );
    }

    // Entrées audio : micro (1), puis son système. Silence si aucune (les plateformes exigent une piste audio).
    let sources: Vec<(&String, f32)> = [
        (&c.audio_device, c.mic_volume),
        (&c.system_audio_device, c.system_volume),
    ]
    .into_iter()
    .filter_map(|(d, v)| d.as_ref().map(|n| (n, v)))
    .collect();
    for (name, _) in &sources {
        if name.starts_with("tcp://") {
            p(
                &mut a,
                &[
                    "-thread_queue_size",
                    "512",
                    "-f",
                    "s16le",
                    "-ar",
                    "48000",
                    "-ac",
                    "2",
                    "-i",
                    name,
                ],
            );
        } else {
            let i = format!("audio={name}");
            p(
                &mut a,
                &["-thread_queue_size", "512", "-f", "dshow", "-i", &i],
            );
        }
    }
    // Première entrée audio : 1, ou 2 si la webcam est incrustée.
    let ab = if c.overlay.is_some() { 2 } else { 1 };
    if sources.is_empty() {
        p(&mut a, &["-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo"]);
    }
    let (w, h) = (c.width, c.height);
    let mut graphs: Vec<String> = Vec::new();
    let vmap = match &c.overlay {
        Some(o) => {
            graphs.push(overlay_graph_pre(w, h, o, pre));
            "[v]".to_string()
        }
        None => {
            let vf = format!(
                "{pre}scale={w}:{h}:force_original_aspect_ratio=decrease:flags=fast_bilinear,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,format=yuv420p"
            );
            p(&mut a, &["-vf", &vf]);
            "0:v:0".to_string()
        }
    };
    let amap = match sources.len() {
        0 => format!("{ab}:a:0"),
        1 => {
            let af = format!("aresample=async=1000,volume={:.2}", sources[0].1);
            p(&mut a, &["-af", &af]);
            format!("{ab}:a:0")
        }
        _ => {
            // Chaque flux est recalé puis mixé ; amix répartit le niveau entre les entrées.
            graphs.push(format!(
                "[{ab}:a]aresample=async=1000,volume={:.2}[m];[{}:a]aresample=async=1000,volume={:.2}[s];[m][s]amix=inputs=2:duration=first[a]",
                sources[0].1,
                ab + 1,
                sources[1].1
            ));
            "[a]".to_string()
        }
    };
    if !graphs.is_empty() {
        p(&mut a, &["-filter_complex", &graphs.join(";")]);
    }
    p(&mut a, &["-map", &vmap, "-map", &amap]);

    match c.encoder {
        Encoder::Nvenc => p(
            &mut a,
            &[
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p4",
                "-tune",
                "ll",
                "-rc",
                "cbr",
                "-b:v",
                &bitrate,
                "-maxrate",
                &bitrate,
                "-bufsize",
                &bufsize,
                "-g",
                &gop,
                "-bf",
                "0",
                "-profile:v",
                "high",
            ],
        ),
        Encoder::Qsv => p(
            &mut a,
            &[
                "-c:v",
                "h264_qsv",
                "-preset",
                "veryfast",
                "-look_ahead",
                "0",
                "-b:v",
                &bitrate,
                "-maxrate",
                &bitrate,
                "-g",
                &gop,
                "-profile:v",
                "high",
            ],
        ),
        Encoder::Amf => p(
            &mut a,
            &[
                "-c:v",
                "h264_amf",
                "-usage",
                "lowlatency",
                "-rc",
                "cbr",
                "-b:v",
                &bitrate,
                "-maxrate",
                &bitrate,
                "-g",
                &gop,
                "-bf",
                "0",
                "-profile:v",
                "high",
            ],
        ),
        Encoder::X264 => p(
            &mut a,
            &[
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-tune",
                "zerolatency",
                "-b:v",
                &bitrate,
                "-maxrate",
                &bitrate,
                "-bufsize",
                &bufsize,
                "-g",
                &gop,
                "-profile:v",
                "high",
                "-x264-params",
                "nal-hrd=cbr",
            ],
        ),
    }
    p(
        &mut a,
        &["-c:a", "aac", "-b:a", "128k", "-ar", "44100", "-ac", "2"],
    );

    // 1 RTMP sans enregistrement : sortie directe. Sinon un seul encodage, N copies via tee.
    match (&c.record_path, c.outputs.len()) {
        (None, 1) => p(&mut a, &["-f", "flv", &c.outputs[0]]),
        (rec, n) => {
            // onfail=ignore : une destination qui tombe ne coupe pas les autres.
            // Exception : 1 RTMP + MKV -> abort, pour que FFmpeg s'arrête et que la reconnexion se déclenche.
            let policy = if rec.is_some() && n == 1 {
                "abort"
            } else {
                "ignore"
            };
            let mut slaves: Vec<String> = c
                .outputs
                .iter()
                .map(|u| format!("[f=flv:onfail={policy}]{u}"))
                .collect();
            if let Some(path) = rec {
                // Une panne disque ne doit jamais couper le direct.
                slaves.push(format!("[f=matroska:onfail=ignore]{path}"));
            }
            let spec = slaves.join("|");
            p(&mut a, &["-flags", "+global_header", "-f", "tee", &spec]);
        }
    }
    Ok(a)
}

/// Masque la clé de flux (dernier segment de chaque URL) avant tout log.
pub fn redact(arg: &str) -> String {
    if !arg.contains("rtmp") {
        return arg.to_string();
    }
    arg.split('|')
        .map(|part| match (part.contains("rtmp"), part.rfind('/')) {
            (true, Some(i)) => format!("{}/***", &part[..i]),
            _ => part.to_string(),
        })
        .collect::<Vec<_>>()
        .join("|")
}

pub fn redact_args(args: &[String]) -> Vec<String> {
    args.iter().map(|s| redact(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(n: usize) -> Config {
        let outs = (0..n)
            .map(|i| format!("rtmp://h{i}.example/app/KEY{i}"))
            .collect();
        Config::new(Source::Screen, outs)
    }

    #[test]
    fn une_destination_flv_direct() {
        let a = build_args(&cfg(1)).unwrap();
        assert_eq!(
            &a[a.len() - 3..],
            ["-f", "flv", "rtmp://h0.example/app/KEY0"]
        );
        assert!(a.contains(&"libx264".to_string()));
    }

    #[test]
    fn plusieurs_destinations_tee_onfail() {
        let a = build_args(&cfg(3)).unwrap();
        let spec = a.last().unwrap();
        assert!(a.contains(&"tee".to_string()));
        assert_eq!(spec.matches("onfail=ignore").count(), 3);
        assert_eq!(spec.matches('|').count(), 2);
    }

    #[test]
    fn fenetre_et_webcam() {
        let mut c = cfg(1);
        c.source = Source::Window("Ma Fenêtre".into());
        assert!(build_args(&c)
            .unwrap()
            .contains(&"title=Ma Fenêtre".to_string()));
        c.source = Source::Webcam("Cam".into());
        c.audio_device = Some("Micro".into());
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"video=Cam".to_string()) && a.contains(&"audio=Micro".to_string()));
    }

    #[test]
    fn validations() {
        assert!(build_args(&Config::new(Source::Screen, vec![])).is_err());
        let mut c = cfg(1);
        c.fps = 120;
        assert!(build_args(&c).is_err());
        let mut c = cfg(1);
        c.outputs = vec!["http://x/y".into()];
        assert!(build_args(&c).is_err());
        let mut c = cfg(1);
        c.outputs = vec!["rtmp://x/y z".into()];
        assert!(build_args(&c).is_err());
        let mut c = cfg(1);
        c.source = Source::Window("  ".into());
        assert!(build_args(&c).is_err());
    }

    #[test]
    fn encodeurs_materiels() {
        for e in [Encoder::Nvenc, Encoder::Qsv, Encoder::Amf] {
            let mut c = cfg(1);
            c.encoder = e;
            assert!(build_args(&c).unwrap().contains(&e.name().to_string()));
        }
    }

    #[test]
    fn redaction_des_cles() {
        let a = build_args(&cfg(2)).unwrap();
        let log = redact_args(&a).join(" ");
        assert!(!log.contains("KEY0") && !log.contains("KEY1"));
        assert!(log.contains("rtmp://h0.example/app/***"));
    }

    #[test]
    fn audio_aucune_source_silence() {
        let a = build_args(&cfg(1)).unwrap();
        assert!(a.iter().any(|s| s.starts_with("anullsrc")));
        assert!(!a.contains(&"-filter_complex".to_string()));
    }

    #[test]
    fn audio_micro_seul_avec_volume() {
        let mut c = cfg(1);
        c.audio_device = Some("Micro".into());
        c.mic_volume = 1.5;
        let a = build_args(&c).unwrap();
        assert!(!a.iter().any(|s| s.starts_with("anullsrc")));
        assert!(a.contains(&"aresample=async=1000,volume=1.50".to_string()));
        assert!(!a.contains(&"-filter_complex".to_string()));
    }

    #[test]
    fn audio_micro_et_systeme_mixes() {
        let mut c = cfg(1);
        c.audio_device = Some("Micro".into());
        c.system_audio_device = Some("Stereo Mix".into());
        c.system_volume = 0.5;
        let a = build_args(&c).unwrap();
        assert!(
            a.contains(&"audio=Micro".to_string()) && a.contains(&"audio=Stereo Mix".to_string())
        );
        let i = a.iter().position(|s| s == "-filter_complex").unwrap();
        assert!(a[i + 1].contains("amix=inputs=2") && a[i + 1].contains("volume=0.50"));
        assert!(a.contains(&"[a]".to_string()));
    }

    #[test]
    fn audio_systeme_seul() {
        let mut c = cfg(1);
        c.system_audio_device = Some("Stereo Mix".into());
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"audio=Stereo Mix".to_string()));
        assert!(!a.contains(&"-filter_complex".to_string()));
    }

    #[test]
    fn audio_systeme_loopback_tcp() {
        let mut c = cfg(1);
        c.system_audio_device = Some("tcp://127.0.0.1:54321".into());
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"tcp://127.0.0.1:54321".to_string()));
        assert!(a.contains(&"s16le".to_string()));
        assert!(a.contains(&"48000".to_string()));
        assert!(!a.contains(&"-filter_complex".to_string()));
    }

    #[test]
    fn audio_validations() {
        let mut c = cfg(1);
        c.audio_device = Some("X".into());
        c.system_audio_device = Some("X".into());
        assert!(build_args(&c).is_err());
        let mut c = cfg(1);
        c.mic_volume = 3.0;
        assert!(build_args(&c).is_err());
        let mut c = cfg(1);
        c.mic_volume = f32::NAN;
        assert!(build_args(&c).is_err());
    }

    #[test]
    fn parse_ancien_format() {
        let s = "[dshow @ 0] DirectShow video devices (some may be both video and audio devices)\n\
                 [dshow @ 0]  \"Integrated Camera\"\n\
                 [dshow @ 0]     Alternative name \"@device_pnp_x\"\n\
                 [dshow @ 0] DirectShow audio devices\n\
                 [dshow @ 0]  \"Microphone (Realtek)\"\n\
                 [dshow @ 0]     Alternative name \"@device_cm_y\"\n\
                 [dshow @ 0]  \"Stereo Mix (Realtek)\"\n";
        let d = parse_dshow_devices(s);
        assert_eq!(d.video, vec!["Integrated Camera"]);
        assert_eq!(
            d.audio,
            vec!["Microphone (Realtek)", "Stereo Mix (Realtek)"]
        );
    }

    #[test]
    fn parse_nouveau_format() {
        let s = "[dshow @ 0] \"Integrated Camera\" (video)\n\
                 [dshow @ 0]   Alternative name \"@device_pnp_x\"\n\
                 [dshow @ 0] \"Microphone (USB)\" (audio)\n\
                 [dshow @ 0] \"Capteur\" (none)\n";
        let d = parse_dshow_devices(s);
        assert_eq!(d.video, vec!["Integrated Camera"]);
        assert_eq!(d.audio, vec!["Microphone (USB)"]);
    }

    fn cfg_overlay(corner: Corner) -> Config {
        let mut c = cfg(1);
        c.overlay = Some(Overlay {
            webcam: "Cam".into(),
            corner,
            size_pct: 25,
            margin_px: 20,
        });
        c
    }

    #[test]
    fn incrustation_entrees_et_graphe() {
        let a = build_args(&cfg_overlay(Corner::BottomRight)).unwrap();
        assert!(a.contains(&"video=Cam".to_string()));
        let i = a.iter().position(|s| s == "-filter_complex").unwrap();
        let g = &a[i + 1];
        assert!(g.contains("overlay=main_w-overlay_w-20:main_h-overlay_h-20"));
        assert!(g.contains("scale=480:-2"));
        assert!(a.contains(&"[v]".to_string()));
        assert!(!a.contains(&"-vf".to_string()));
        assert!(a.iter().any(|s| s.starts_with("anullsrc")));
        assert!(a.contains(&"2:a:0".to_string()));
    }

    #[test]
    fn incrustation_coins() {
        let mk = |corner| Overlay {
            webcam: "C".into(),
            corner,
            size_pct: 25,
            margin_px: 10,
        };
        assert!(overlay_graph(1920, 1080, &mk(Corner::TopLeft)).contains("overlay=10:10"));
        assert!(overlay_graph(1920, 1080, &mk(Corner::TopRight))
            .contains("overlay=main_w-overlay_w-10:10"));
        assert!(overlay_graph(1920, 1080, &mk(Corner::BottomLeft))
            .contains("overlay=10:main_h-overlay_h-10"));
    }

    #[test]
    fn incrustation_avec_audio_mixe() {
        let mut c = cfg_overlay(Corner::TopLeft);
        c.audio_device = Some("Micro".into());
        c.system_audio_device = Some("Stereo Mix".into());
        let a = build_args(&c).unwrap();
        assert_eq!(a.iter().filter(|s| *s == "-filter_complex").count(), 1);
        let i = a.iter().position(|s| s == "-filter_complex").unwrap();
        assert!(
            a[i + 1].contains("[2:a]")
                && a[i + 1].contains("[3:a]")
                && a[i + 1].contains("amix=inputs=2")
        );
    }

    #[test]
    fn incrustation_validations() {
        let mut c = cfg_overlay(Corner::TopLeft);
        c.source = Source::Webcam("X".into());
        assert!(build_args(&c).is_err());
        for size in [2, 80] {
            let mut c = cfg_overlay(Corner::TopLeft);
            c.overlay.as_mut().unwrap().size_pct = size;
            assert!(build_args(&c).is_err());
        }
        let mut c = cfg_overlay(Corner::TopLeft);
        c.overlay.as_mut().unwrap().margin_px = 500;
        assert!(build_args(&c).is_err());
        let mut c = cfg_overlay(Corner::TopLeft);
        c.overlay.as_mut().unwrap().webcam = "  ".into();
        assert!(build_args(&c).is_err());
    }

    #[test]
    fn incrustation_un_seul_audio() {
        let mut c = cfg_overlay(Corner::TopLeft);
        c.audio_device = Some("Micro".into());
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"-af".to_string()));
        assert!(a.contains(&"2:a:0".to_string()));
        assert!(!a.iter().any(|s| s.starts_with("anullsrc")));
    }

    #[test]
    fn mkv_une_destination_abort() {
        let mut c = cfg(1);
        c.record_path = Some("C:/Videos/live.mkv".into());
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"tee".to_string()));
        assert_eq!(
            a.last().unwrap().as_str(),
            "[f=flv:onfail=abort]rtmp://h0.example/app/KEY0|[f=matroska:onfail=ignore]C:/Videos/live.mkv"
        );
    }

    #[test]
    fn mkv_plusieurs_destinations_ignore() {
        let mut c = cfg(3);
        c.record_path = Some("C:/Videos/live.mkv".into());
        let spec = build_args(&c).unwrap().pop().unwrap();
        assert_eq!(spec.matches("onfail=ignore").count(), 4);
        assert_eq!(spec.matches("onfail=abort").count(), 0);
        assert_eq!(spec.matches('|').count(), 3);
    }

    #[test]
    fn mkv_validations() {
        for bad in [
            "",
            "  ",
            "C:/v/live.mp4",
            "C:/v/a|b.mkv",
            "C:\\v\\live.mkv",
            "C:/v/[x].mkv",
            "C:/v/o'x.mkv",
        ] {
            let mut c = cfg(1);
            c.record_path = Some(bad.into());
            assert!(build_args(&c).is_err(), "devrait échouer : {bad}");
        }
    }

    #[test]
    fn mkv_masquage_cle_chemin_intact() {
        let mut c = cfg(1);
        c.record_path = Some("C:/Videos/live.mkv".into());
        let r = redact_args(&build_args(&c).unwrap());
        let spec = r.last().unwrap();
        assert!(spec.contains("rtmp://h0.example/app/***"));
        assert!(!spec.contains("KEY0"));
        assert!(spec.ends_with("[f=matroska:onfail=ignore]C:/Videos/live.mkv"));
    }

    #[test]
    fn mkv_avec_incrustation() {
        let mut c = cfg(1);
        c.record_path = Some("C:/Videos/live.mkv".into());
        c.overlay = Some(Overlay {
            webcam: "Cam".into(),
            corner: Corner::TopLeft,
            size_pct: 25,
            margin_px: 20,
        });
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"-filter_complex".to_string()));
        assert!(a.last().unwrap().contains("matroska"));
    }

    #[test]
    fn ecran_ddagrab() {
        let mut c = cfg(1);
        c.source = Source::Screen;
        c.use_ddagrab = true;
        let a = build_args(&c).unwrap();
        assert!(a.iter().any(|s| s.starts_with("ddagrab=framerate=")));
        assert!(!a.contains(&"gdigrab".to_string()));
        let i = a.iter().position(|s| s == "-vf").unwrap();
        assert!(a[i + 1].starts_with("hwdownload,format=bgra,scale="));
    }

    #[test]
    fn ecran_ddagrab_overlay() {
        let mut c = cfg(1);
        c.source = Source::Screen;
        c.use_ddagrab = true;
        c.overlay = Some(Overlay {
            webcam: "Cam".into(),
            corner: Corner::TopLeft,
            size_pct: 25,
            margin_px: 20,
        });
        c.audio_device = Some("Micro".into());
        c.system_audio_device = Some("Stereo Mix".into());
        let a = build_args(&c).unwrap();
        assert!(a.iter().any(|s| s.starts_with("ddagrab=framerate=")));
        let i = a.iter().position(|s| s == "-filter_complex").unwrap();
        assert!(a[i + 1].starts_with("[0:v]hwdownload,format=bgra,scale="));
        assert!(a[i + 1].contains("[2:a]") && a[i + 1].contains("[3:a]"));
    }

    #[test]
    fn fenetre_ignore_ddagrab() {
        let mut c = cfg(1);
        c.source = Source::Window("W".into());
        c.use_ddagrab = true;
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"gdigrab".to_string()));
        assert!(!a.iter().any(|s| s.contains("ddagrab")));
    }

    #[test]
    fn region_ddagrab() {
        let mut c = cfg(1);
        c.source = Source::Region {
            x: 100,
            y: 120,
            width: 1650,
            height: 950,
        };
        c.use_ddagrab = true;
        let a = build_args(&c).unwrap();
        assert!(a.iter().any(|s| s.contains("ddagrab=framerate=")
            && s.contains("offset_x=100")
            && s.contains("video_size=1650x950")));
        let i = a.iter().position(|s| s == "-vf").unwrap();
        assert!(a[i + 1].starts_with("hwdownload,format=bgra,scale="));
    }

    #[test]
    fn region_gdigrab() {
        let mut c = cfg(1);
        c.source = Source::Region {
            x: 50,
            y: 80,
            width: 1280,
            height: 720,
        };
        c.use_ddagrab = false;
        let a = build_args(&c).unwrap();
        assert!(a.contains(&"-offset_x".to_string()));
        assert!(a.contains(&"50".to_string()));
        assert!(a.contains(&"-offset_y".to_string()));
        assert!(a.contains(&"80".to_string()));
        assert!(a.contains(&"-video_size".to_string()));
        assert!(a.contains(&"1280x720".to_string()));
        assert!(a.contains(&"desktop".to_string()));
    }

    #[test]
    fn pause_stream() {
        let mut c = cfg(1);
        c.source = Source::Pause(String::new());
        let a = build_args(&c).unwrap();
        assert!(a
            .iter()
            .any(|s| s.starts_with("color=c=#14151a:s=1920x1080")));
    }
}
