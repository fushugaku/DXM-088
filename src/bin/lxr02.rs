use lxr02::{Engine, MAX_FRAMES};
use std::{
    env, fs,
    io::{self, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path},
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("serve") {
        "serve"=>serve(args.get(1).map(|s|s.parse()).transpose()?.unwrap_or(8732))?,
        "render"=>render(&args[1..])?,
        _=>return Err("Usage: dxm088 serve [port] | dxm088 render out.wav [--bars N --bpm N --kit 0..4 --sample-rate N]".into()),
    }
    Ok(())
}
fn serve(port: u16) -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("web")
        .canonicalize()?;
    if !root.join("lxr02.wasm").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Run ./run.sh to build the WASM audio engine first",
        ));
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("DXM-088 → http://127.0.0.1:{port}");
    println!("Rust DSP · AudioWorklet · Ctrl+C stops the server");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root.clone();
                std::thread::spawn(move || {
                    let _ = response(stream, &root);
                });
            }
            Err(err) => eprintln!("{err}"),
        }
    }
    Ok(())
}
fn response(mut stream: TcpStream, root: &Path) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        request.extend_from_slice(&chunk[..n]);
        if request.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if request.len() > 8192 {
            return Ok(());
        }
    }
    let line = String::from_utf8_lossy(&request);
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return send(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"Method not allowed",
            false,
        );
    }
    let path = target
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_start_matches('/');
    let relative = if path.is_empty() { "index.html" } else { path };
    if relative.contains('%')
        || relative.contains('\\')
        || Path::new(relative)
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
    {
        return send(
            &mut stream,
            "400 Bad Request",
            "text/plain",
            b"Invalid path",
            method == "HEAD",
        );
    }
    let candidate = root.join(relative);
    let resolved = match candidate.canonicalize() {
        Ok(p) if p.starts_with(root) && p.is_file() => p,
        _ => {
            return send(
                &mut stream,
                "404 Not Found",
                "text/plain",
                b"Not found",
                method == "HEAD",
            )
        }
    };
    let content = fs::read(&resolved)?;
    let mime = match resolved.extension().and_then(|x| x.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        _ => "application/octet-stream",
    };
    send(&mut stream, "200 OK", mime, &content, method == "HEAD")
}
fn send(
    stream: &mut TcpStream,
    status: &str,
    mime: &str,
    data: &[u8],
    head: bool,
) -> io::Result<()> {
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",data.len())?;
    if !head {
        stream.write_all(data)?;
    }
    Ok(())
}
fn render(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = args.first().ok_or("render requires an output WAV path")?;
    let mut bars = 4usize;
    let mut bpm = 128f32;
    let mut kit = 0usize;
    let mut sr = 44100u32;
    for pair in args[1..].chunks(2) {
        let value = pair.get(1).ok_or("missing option value")?;
        match pair[0].as_str() {
            "--bars" => bars = value.parse()?,
            "--bpm" => bpm = value.parse()?,
            "--kit" => kit = value.parse()?,
            "--sample-rate" => sr = value.parse()?,
            _ => return Err("unknown render option".into()),
        }
    }
    if !(1..=256).contains(&bars)
        || !(30. ..=300.).contains(&bpm)
        || !(8000..=192000).contains(&sr)
        || kit > 4
    {
        return Err("render parameters out of range".into());
    }
    let music = (bars as f64 * 4. * 60. / bpm as f64 * sr as f64).round() as usize;
    let total = music + sr as usize * 2;
    if total as u64 * 4 > u32::MAX as u64 - 36 {
        return Err("WAV too large".into());
    }
    let mut writer = BufWriter::new(fs::File::create(file)?);
    writer.write_all(b"RIFF")?;
    writer.write_all(&((total * 4 + 36) as u32).to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    for n in [1u16, 2u16] {
        writer.write_all(&n.to_le_bytes())?;
    }
    writer.write_all(&sr.to_le_bytes())?;
    writer.write_all(&(sr * 4).to_le_bytes())?;
    writer.write_all(&4u16.to_le_bytes())?;
    writer.write_all(&16u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&((total * 4) as u32).to_le_bytes())?;
    let mut e = Engine::new(sr as f32);
    e.load_kit(kit);
    e.tempo(bpm, 0.);
    e.play();
    let mut offset = 0;
    let mut peak = 0f32;
    let mut squares = 0f64;
    while offset < total {
        if offset == music {
            e.stop();
        }
        let boundary = if offset < music { music } else { total };
        let n = (boundary - offset).min(MAX_FRAMES);
        for &x in e.render(n) {
            peak = peak.max(x.abs());
            squares += (x as f64).powi(2);
            writer.write_all(&((x.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes())?;
        }
        offset += n;
    }
    writer.flush()?;
    println!(
        "Rendered {file}: stereo {sr} Hz, {:.2} s, peak {peak:.4}, RMS {:.4}",
        total as f64 / sr as f64,
        (squares / (total * 2) as f64).sqrt()
    );
    Ok(())
}
