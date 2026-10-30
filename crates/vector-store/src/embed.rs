//! chromadb's default embedding function (`ONNXMiniLM_L6_V2`): all-MiniLM-L6-v2 in ONNX,
//! downloaded once into `~/.cache/chroma/onnx_models/all-MiniLM-L6-v2`, the cache the
//! Python package uses. Texts are tokenized to 256 tokens, run through the model, mean
//! pooled over the attention mask and L2 normalised.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use tract_onnx::prelude::*;

use crate::StoreError;

pub const MODEL_NAME: &str = "all-MiniLM-L6-v2";
const ARCHIVE_FILENAME: &str = "onnx.tar.gz";
const MODEL_DOWNLOAD_URL: &str =
    "https://chroma-onnx-models.s3.amazonaws.com/all-MiniLM-L6-v2/onnx.tar.gz";
const MODEL_SHA256: &str = "913d7300ceae3b2dbc2c50d1de4baacab4be7b9380491c27fab7418616a16ec3";
const ONNX_FILES: [&str; 6] = [
    "config.json",
    "model.onnx",
    "special_tokens_map.json",
    "tokenizer_config.json",
    "tokenizer.json",
    "vocab.txt",
];
const MAX_TOKENS: usize = 256;
const BATCH_SIZE: usize = 32;
pub const DIMENSION: usize = 384;

type Model = std::sync::Arc<TypedRunnableModel>;

pub struct Embedder {
    tokenizer: tokenizers::Tokenizer,
    model: Model,
}

/// `Path.home()`.
pub fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn download_path() -> PathBuf {
    home()
        .join(".cache")
        .join("chroma")
        .join("onnx_models")
        .join(MODEL_NAME)
}

fn sha256_file(path: &Path) -> Result<String, StoreError> {
    Ok(kernel::sha256::hex(&fs::read(path)?))
}

/// Human sizes as tqdm writes them with `unit_scale` and `unit_divisor=1024`.
fn tqdm_size(n: f64) -> String {
    let mut n = n;
    for unit in ["", "k", "M", "G", "T"] {
        if n.abs() < 999.5 {
            if n.abs() < 99.95 {
                if n.abs() < 9.995 {
                    return format!("{n:1.2}{unit}");
                }
                return format!("{n:2.1}{unit}");
            }
            return format!("{n:3.0}{unit}");
        }
        n /= 1024.0;
    }
    format!("{n:3.1}P")
}

fn tqdm_line(desc: &str, done: u64, total: u64, start: Instant) -> String {
    let elapsed = start.elapsed().as_secs_f64();
    let rate = if elapsed > 0.0 {
        done as f64 / elapsed
    } else {
        0.0
    };
    let clock = |s: f64| {
        let s = s as u64;
        format!("{:02}:{:02}", s / 60, s % 60)
    };
    let remaining = if rate > 0.0 && total > done {
        (total - done) as f64 / rate
    } else {
        0.0
    };
    let pct = if total > 0 {
        done as f64 * 100.0 / total as f64
    } else {
        0.0
    };
    let filled = (done * 10).checked_div(total).unwrap_or(0) as usize;
    let bar: String = "█".repeat(filled) + &" ".repeat(10 - filled.min(10));
    format!(
        "{desc}: {pct:3.0}%|{bar}| {}/{} [{}<{}, {}iB/s]",
        tqdm_size(done as f64),
        tqdm_size(total as f64),
        clock(elapsed),
        clock(remaining),
        tqdm_size(rate)
    )
}

fn agent() -> Result<ureq::Agent, StoreError> {
    let mut tls = ureq::tls::TlsConfig::builder();
    if let Some(file) = std::env::var_os("SSL_CERT_FILE") {
        let pem = fs::read(&file)?;
        let certs: Vec<ureq::tls::Certificate<'static>> = ureq::tls::parse_pem(&pem)
            .filter_map(|item| match item {
                Ok(ureq::tls::PemItem::Certificate(c)) => Some(c),
                _ => None,
            })
            .collect();
        tls = tls.root_certs(ureq::tls::RootCerts::new_with_certs(&certs));
    }
    let config = ureq::Agent::config_builder()
        .tls_config(tls.build())
        .build();
    Ok(ureq::Agent::new_with_config(config))
}

/// `_download`: streams the archive with a tqdm-style bar on stderr, then checks it.
fn download(fname: &Path) -> Result<(), StoreError> {
    let desc = fname.display().to_string();
    for attempt in 0..3 {
        let mut resp = agent()?
            .get(MODEL_DOWNLOAD_URL)
            .call()
            .map_err(|e| StoreError::new(format!("Download failed: {e}")))?;
        let total: u64 = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut reader = resp.body_mut().as_reader();
        let mut file = fs::File::create(fname)?;
        let start = Instant::now();
        let mut done: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];
        let mut last = Instant::now();
        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "\r{}", tqdm_line(&desc, 0, total, start));
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            done += n as u64;
            if last.elapsed().as_millis() >= 100 {
                let _ = write!(stderr, "\r{}", tqdm_line(&desc, done, total, start));
                last = Instant::now();
            }
        }
        let _ = writeln!(stderr, "\r{}", tqdm_line(&desc, done, total, start));
        drop(file);
        if sha256_file(fname)? == MODEL_SHA256 {
            return Ok(());
        }
        let _ = fs::remove_file(fname);
        if attempt == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1000 + (done % 2000)));
    }
    Err(StoreError::new(format!(
        "Downloaded file {} does not match expected SHA256 hash. Corrupted download or malicious file.",
        fname.display()
    )))
}

/// `_download_model_if_not_exists`.
pub fn ensure_model() -> Result<PathBuf, StoreError> {
    let base = download_path();
    let extracted = base.join("onnx");
    if ONNX_FILES.iter().all(|f| extracted.join(f).exists()) {
        return Ok(extracted);
    }
    fs::create_dir_all(&base)?;
    let archive = base.join(ARCHIVE_FILENAME);
    if !archive.exists() || sha256_file(&archive)? != MODEL_SHA256 {
        download(&archive)?;
    }
    let file = fs::File::open(&archive)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    tar.unpack(&base)?;
    Ok(extracted)
}

impl Embedder {
    pub fn load() -> Result<Embedder, StoreError> {
        let dir = ensure_model()?;
        let mut tokenizer = tokenizers::Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| StoreError::new(format!("tokenizer: {e}")))?;
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(|e| StoreError::new(format!("tokenizer: {e}")))?;
        tokenizer.with_padding(Some(tokenizers::PaddingParams {
            strategy: tokenizers::PaddingStrategy::Fixed(MAX_TOKENS),
            pad_id: 0,
            pad_token: "[PAD]".into(),
            ..Default::default()
        }));
        let tract_err = |e: TractError| StoreError::new(format!("onnx: {e}"));
        let onnx = tract_onnx::onnx();
        let mut model = onnx
            .model_for_path(dir.join("model.onnx"))
            .map_err(tract_err)?;
        let batch = model.symbols.sym("N");
        for i in 0..3 {
            model
                .set_input_fact(
                    i,
                    i64::fact([TDim::from(batch.clone()), TDim::from(MAX_TOKENS as i64)]).into(),
                )
                .map_err(tract_err)?;
        }
        let model = model
            .into_optimized()
            .map_err(tract_err)?
            .into_runnable()
            .map_err(tract_err)?;
        Ok(Embedder { tokenizer, model })
    }

    /// `_forward`: the normalised embeddings, in batches of 32.
    pub fn embed(&self, documents: &[String]) -> Result<Vec<Vec<f32>>, StoreError> {
        let tract_err = |e: TractError| StoreError::new(format!("onnx: {e}"));
        let mut out = Vec::with_capacity(documents.len());
        for batch in documents.chunks(BATCH_SIZE) {
            let n = batch.len();
            let mut ids = Vec::with_capacity(n * MAX_TOKENS);
            let mut mask = Vec::with_capacity(n * MAX_TOKENS);
            for d in batch {
                let enc = self
                    .tokenizer
                    .encode(d.as_str(), true)
                    .map_err(|e| StoreError::new(format!("tokenizer: {e}")))?;
                if enc.get_ids().len() > MAX_TOKENS {
                    return Err(StoreError::new(format!(
                        "Document length {} is greater than the max tokens {MAX_TOKENS}",
                        enc.get_ids().len()
                    )));
                }
                ids.extend(enc.get_ids().iter().map(|&x| x as i64));
                mask.extend(enc.get_attention_mask().iter().map(|&x| x as i64));
            }
            let shape = (n, MAX_TOKENS);
            let a = tract_ndarray::Array2::from_shape_vec(shape, ids)
                .map_err(|e| StoreError::new(e.to_string()))?;
            let m = tract_ndarray::Array2::from_shape_vec(shape, mask.clone())
                .map_err(|e| StoreError::new(e.to_string()))?;
            let t = tract_ndarray::Array2::<i64>::zeros(shape);
            let result = self
                .model
                .run(tvec!(
                    a.into_tensor().into(),
                    m.into_tensor().into(),
                    t.into_tensor().into()
                ))
                .map_err(tract_err)?;
            let hidden = result[0].to_plain_array_view::<f32>().map_err(tract_err)?;
            let width = hidden.shape()[2];
            for i in 0..n {
                let mut v = vec![0f32; width];
                let mut count = 0f32;
                for j in 0..MAX_TOKENS {
                    if mask[i * MAX_TOKENS + j] != 0 {
                        count += 1.0;
                        for (k, x) in v.iter_mut().enumerate() {
                            *x += hidden[[i, j, k]];
                        }
                    }
                }
                let count = count.max(1e-9);
                for x in v.iter_mut() {
                    *x /= count;
                }
                let mut norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm == 0.0 {
                    norm = 1e-12;
                }
                out.push(v.iter().map(|x| x / norm).collect());
            }
        }
        Ok(out)
    }
}
