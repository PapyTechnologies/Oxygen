use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct LoopbackHandle {
    pub port: u16,
    running: Arc<AtomicBool>,
    _worker: Option<JoinHandle<()>>,
}

impl LoopbackHandle {
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for LoopbackHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_loopback() -> Result<LoopbackHandle, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Impossible de lier le serveur TCP audio loopback : {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking impossible : {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local_addr impossible : {e}"))?
        .port();

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = running.clone();

    let (tx_ready, rx_ready) = std::sync::mpsc::channel::<Result<(), String>>();

    let worker = thread::spawn(move || {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                let _ = tx_ready.send(Err(
                    "Aucun périphérique audio de sortie (haut-parleur / casque) trouvé".into(),
                ));
                return;
            }
        };

        let supported_config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx_ready.send(Err(format!(
                    "Configuration du périphérique audio impossible : {e}"
                )));
                return;
            }
        };

        let in_sr = supported_config.sample_rate().0 as usize;
        let in_ch = supported_config.channels() as usize;
        let sample_format = supported_config.sample_format();

        let buffer = Arc::new(Mutex::new(Vec::<u8>::with_capacity(3840 * 10)));
        let buf_writer = buffer.clone();

        // Fonction de conversion et rééchantillonnage vers 48000 Hz stéréo s16le
        let process_samples = move |samples: &[f32], buf: &Arc<Mutex<Vec<u8>>>| {
            if in_ch == 0 || in_sr == 0 {
                return;
            }

            let num_frames = samples.len() / in_ch;
            let mut stereo_in = Vec::with_capacity(num_frames * 2);
            for frame in samples.chunks_exact(in_ch) {
                let left = frame[0];
                let right = if in_ch > 1 { frame[1] } else { frame[0] };
                stereo_in.push((left, right));
            }

            let mut stereo_48k = Vec::new();
            if in_sr == 48000 {
                for (l, r) in stereo_in {
                    let sl = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
                    let sr = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
                    stereo_48k.extend_from_slice(&sl.to_le_bytes());
                    stereo_48k.extend_from_slice(&sr.to_le_bytes());
                }
            } else {
                let ratio = in_sr as f64 / 48000.0;
                let out_frames = ((num_frames as f64) / ratio) as usize;
                for i in 0..out_frames {
                    let pos = i as f64 * ratio;
                    let idx = pos as usize;
                    let frac = (pos - idx as f64) as f32;
                    let (l0, r0) = stereo_in[idx.min(num_frames - 1)];
                    let (l1, r1) = stereo_in[(idx + 1).min(num_frames - 1)];
                    let l = l0 + (l1 - l0) * frac;
                    let r = r0 + (r1 - r0) * frac;
                    let sl = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
                    let sr = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
                    stereo_48k.extend_from_slice(&sl.to_le_bytes());
                    stereo_48k.extend_from_slice(&sr.to_le_bytes());
                }
            }

            if let Ok(mut g) = buf.lock() {
                if g.len() > 192000 {
                    g.drain(0..96000);
                }
                g.extend_from_slice(&stereo_48k);
            }
        };

        let stream_res = match sample_format {
            cpal::SampleFormat::F32 => {
                let buf_cb = buf_writer;
                device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        process_samples(data, &buf_cb);
                    },
                    |err| eprintln!("[audio_loopback] Erreur stream F32: {err}"),
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let buf_cb = buf_writer;
                device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let f32_samples: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 32768.0).collect();
                        process_samples(&f32_samples, &buf_cb);
                    },
                    |err| eprintln!("[audio_loopback] Erreur stream I16: {err}"),
                    None,
                )
            }
            _ => {
                let _ = tx_ready.send(Err(format!(
                    "Format d'échantillon non supporté : {:?}",
                    sample_format
                )));
                return;
            }
        };

        let stream = match stream_res {
            Ok(s) => s,
            Err(e) => {
                let _ = tx_ready.send(Err(format!("build_input_stream impossible : {e}")));
                return;
            }
        };

        if let Err(e) = stream.play() {
            let _ = tx_ready.send(Err(format!("play audio stream impossible : {e}")));
            return;
        }

        let _ = tx_ready.send(Ok(()));

        let chunk_size = 3840; // 20 ms @ 48000 Hz stéréo 16-bit (960 échantillons stéréo * 4 octets)
        let silence = vec![0u8; chunk_size];

        while r_clone.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    let _ = socket.set_nodelay(true);
                    let _ = socket.set_nonblocking(false);
                    let mut tick = Instant::now();

                    while r_clone.load(Ordering::SeqCst) {
                        let data = {
                            let mut g = buffer.lock().unwrap();
                            if g.len() >= chunk_size {
                                g.drain(0..chunk_size).collect::<Vec<u8>>()
                            } else {
                                silence.clone()
                            }
                        };

                        if socket.write_all(&data).is_err() {
                            break;
                        }

                        let elapsed = tick.elapsed();
                        let target = Duration::from_millis(20);
                        if elapsed < target {
                            thread::sleep(target - elapsed);
                        }
                        tick = Instant::now();
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => {
                    break;
                }
            }
        }
        // stream est droppé ici dans le thread worker à la fin
    });

    rx_ready
        .recv()
        .map_err(|e| format!("Erreur initialisation thread loopback : {e}"))??;

    Ok(LoopbackHandle {
        port,
        running,
        _worker: Some(worker),
    })
}
