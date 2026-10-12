//! The model inside the Windows installer. NSIS can't hold more than about
//! 2 GB of data, and the larger models are 2.5–3.4 GB, so the release
//! script appends the model to the finished setup .exe instead
//! (`tools/release/append-model.py`): model bytes, then their length as a
//! little-endian u64, then `MAGIC`. Authenticode signing, when it is turned
//! on, happens after that and adds its certificate table at the very end,
//! so the footer is looked for just before that table.
//!
//! The installer's post-install hook runs `<app> --install-model <setup.exe>`,
//! which copies the model into `<install dir>\model\`, checks its SHA-256
//! and only then gives it its real name. (DECISIONS.md, Phase 6.)

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8; 8] = b"SPOTMDL1";
const FOOTER: u64 = 16;

/// Where the payload sits in `setup`: (start, length).
pub fn locate(setup: &Path) -> Result<(u64, u64), String> {
    let mut f = File::open(setup).map_err(|e| format!("can't open the installer: {e}"))?;
    let file_len = f.metadata().map_err(|e| e.to_string())?.len();
    let end = match certificate_table(&mut f)? {
        Some((offset, size)) if offset + size == file_len && offset <= file_len => offset,
        Some(_) => return Err("the installer's signature isn't where it should be".into()),
        None => file_len,
    };
    if end < FOOTER {
        return Err("the installer carries no model".into());
    }
    let mut footer = [0u8; FOOTER as usize];
    f.seek(SeekFrom::Start(end - FOOTER))
        .map_err(|e| e.to_string())?;
    f.read_exact(&mut footer).map_err(|e| e.to_string())?;
    if &footer[8..] != MAGIC {
        return Err("the installer carries no model".into());
    }
    let len = u64::from_le_bytes(footer[..8].try_into().expect("8 bytes"));
    let start = (end - FOOTER)
        .checked_sub(len)
        .ok_or("the installer is damaged (model length)")?;
    Ok((start, len))
}

/// The Authenticode certificate table (security data directory) of a PE
/// file, as (file offset, size), if it has one.
fn certificate_table(f: &mut File) -> Result<Option<(u64, u64)>, String> {
    let mut read_at = |at: u64, buf: &mut [u8]| -> Result<(), String> {
        f.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
        f.read_exact(buf)
            .map_err(|_| "the installer isn't a Windows program".to_string())
    };
    let mut mz = [0u8; 64];
    read_at(0, &mut mz)?;
    if &mz[..2] != b"MZ" {
        return Err("the installer isn't a Windows program".into());
    }
    let pe = u32::from_le_bytes(mz[0x3c..0x40].try_into().expect("4 bytes")) as u64;
    let mut head = [0u8; 24 + 2];
    read_at(pe, &mut head)?;
    if &head[..4] != b"PE\0\0" {
        return Err("the installer isn't a Windows program".into());
    }
    let optional = pe + 24;
    let dirs = match u16::from_le_bytes([head[24], head[25]]) {
        0x10b => optional + 96,  // PE32 (the NSIS stub)
        0x20b => optional + 112, // PE32+
        _ => return Err("the installer isn't a Windows program".into()),
    };
    let mut security = [0u8; 8];
    read_at(dirs + 4 * 8, &mut security)?;
    let offset = u32::from_le_bytes(security[..4].try_into().expect("4 bytes")) as u64;
    let size = u32::from_le_bytes(security[4..].try_into().expect("4 bytes")) as u64;
    Ok((offset != 0 && size != 0).then_some((offset, size)))
}

/// Copies the payload of `setup` to `dest`, checks it against `sha256`, and
/// renames it into place. A half-copied or damaged file is never left under
/// the real name.
pub fn extract(setup: &Path, dest: &Path, sha256: &str) -> Result<u64, String> {
    let (start, len) = locate(setup)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    }
    let part = dest.with_extension("part");
    let copied = (|| -> std::io::Result<u64> {
        let mut from = File::open(setup)?;
        from.seek(SeekFrom::Start(start))?;
        let mut to = File::create(&part)?;
        let n = std::io::copy(&mut from.take(len), &mut to)?;
        to.flush()?;
        to.sync_all()?;
        Ok(n)
    })();
    let fail = |msg: String| {
        let _ = std::fs::remove_file(&part);
        Err(msg)
    };
    match copied {
        Ok(n) if n == len => {}
        Ok(_) => return fail("the installer is damaged (model cut short)".into()),
        Err(e) => return fail(format!("couldn't copy the model: {e}")),
    }
    match spotter_llm::sha256_file(&part) {
        Ok(h) if h == sha256 => {}
        Ok(_) => return fail("the installer is damaged (model checksum)".into()),
        Err(e) => return fail(format!("couldn't check the model: {e}")),
    }
    let _ = std::fs::remove_file(dest);
    std::fs::rename(&part, dest).map_err(|e| format!("couldn't put the model in place: {e}"))?;
    Ok(len)
}

/// `--install-model <setup.exe>`: run by the installer, never by a user.
/// Returns the process exit code.
pub fn install_from_args(setup: &Path) -> i32 {
    if crate::model::VARIANT == "off" {
        return 0;
    }
    let dest: PathBuf = match std::env::current_exe() {
        Ok(exe) => exe
            .parent()
            .unwrap_or(Path::new("."))
            .join("model")
            .join(crate::model::FILE),
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    match extract(setup, &dest, crate::model::SHA256) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("{e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal PE32 header: enough for `certificate_table`.
    fn stub(cert: Option<(u32, u32)>) -> Vec<u8> {
        let mut v = vec![0u8; 0x200];
        v[..2].copy_from_slice(b"MZ");
        v[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        v[0x80..0x84].copy_from_slice(b"PE\0\0");
        v[0x80 + 24..0x80 + 26].copy_from_slice(&0x10bu16.to_le_bytes());
        if let Some((off, size)) = cert {
            let at = 0x80 + 24 + 96 + 32;
            v[at..at + 4].copy_from_slice(&off.to_le_bytes());
            v[at + 4..at + 8].copy_from_slice(&size.to_le_bytes());
        }
        v
    }

    fn with_payload(mut v: Vec<u8>, payload: &[u8]) -> Vec<u8> {
        v.extend_from_slice(payload);
        v.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        v.extend_from_slice(MAGIC);
        v
    }

    fn sha(bytes: &[u8]) -> String {
        let dir = tempdir();
        let p = dir.join("x");
        std::fs::write(&p, bytes).unwrap();
        spotter_llm::sha256_file(&p).unwrap()
    }

    fn tempdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "spotter-payload-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn extracts_and_checks_the_model() {
        let payload = b"pretend this is a 3 GB model".repeat(1000);
        let dir = tempdir();
        let setup = dir.join("setup.exe");
        std::fs::write(&setup, with_payload(stub(None), &payload)).unwrap();
        let dest = dir.join("model").join("m.gguf");
        assert_eq!(
            extract(&setup, &dest, &sha(&payload)),
            Ok(payload.len() as u64)
        );
        assert_eq!(std::fs::read(&dest).unwrap(), payload);
        assert!(!dest.with_extension("part").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn finds_the_model_before_a_signature() {
        let payload = b"model bytes".to_vec();
        let unsigned = with_payload(stub(None), &payload);
        let cert = vec![7u8; 40];
        let mut signed = with_payload(
            stub(Some((unsigned.len() as u32, cert.len() as u32))),
            &payload,
        );
        signed.extend_from_slice(&cert);
        let dir = tempdir();
        let setup = dir.join("setup.exe");
        std::fs::write(&setup, &signed).unwrap();
        assert_eq!(locate(&setup), Ok((0x200, payload.len() as u64)));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_model_never_gets_its_name() {
        let payload = b"model bytes".to_vec();
        let dir = tempdir();
        let setup = dir.join("setup.exe");
        std::fs::write(&setup, with_payload(stub(None), &payload)).unwrap();
        let dest = dir.join("m.gguf");
        let wrong = "0".repeat(64);
        assert!(
            extract(&setup, &dest, &wrong)
                .unwrap_err()
                .contains("checksum")
        );
        assert!(!dest.exists() && !dest.with_extension("part").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_installer_without_a_model_says_so() {
        let dir = tempdir();
        let setup = dir.join("setup.exe");
        std::fs::write(&setup, stub(None)).unwrap();
        assert!(locate(&setup).unwrap_err().contains("no model"));
        std::fs::write(&setup, b"not a program").unwrap();
        assert!(
            locate(&setup)
                .unwrap_err()
                .contains("isn't a Windows program")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
