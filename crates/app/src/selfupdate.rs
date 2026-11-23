//! DbCraft updates itself, as ArtCraft's launcher (https://github.com/storytold/craft-launcher,
//! MIT OR Apache-2.0) and OpenSlate do: it downloads the new release's file, checks its SHA-256
//! checksum (the release's `SHA256SUMS.txt`) and puts it in place of this copy, the same way it
//! was installed. Then, when the user wants, a small process waits for the app to close and
//! opens it again.
//!
//! | Installed as | Swap |
//! |---|---|
//! | macOS `DbCraft.app` | the new app from the DMG copied in place of this one (fine while it runs) |
//! | macOS, bare program (`target/release/dbcraft`) | the program inside the DMG's app in place of this one |
//! | Linux, `.tar.gz` archive | the new files in place of the folder's ones (the running program stays valid) |
//! | Linux, `.deb` package | `pkexec dpkg -i` (the system asks for the password) |
//! | Windows, `.zip` archive | after closing, a PowerShell helper copies the files and reopens the app |
//!
//! Built from source (`cargo run`) it does not update itself (`git pull`), unless asked
//! with `DBCRAFT_UPDATE_TEST=1`: then it updates like a release.
//!
//! The data are not touched: connections and settings live in the user's config folder,
//! outside the app; passwords in the system keychain.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::update::Release;

type Result<T> = std::result::Result<T, String>;

/// How this copy is installed.
#[derive(Clone, Debug, PartialEq)]
pub enum Install {
    /// macOS: the `DbCraft.app` bundle.
    MacApp(PathBuf),
    /// macOS: the program outside an app bundle (e.g. `target/release/dbcraft`).
    MacBinary(PathBuf),
    /// Linux: the .deb package (the program in /usr/bin).
    Deb(PathBuf),
    /// Linux or Windows: the folder extracted from the archive (the program).
    Folder(PathBuf),
}

/// Where the update is at (for the bar).
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Downloading { done: u64, total: u64 },
    Verifying,
    Installing,
}

/// Update installed: the command that reopens the app after it closes.
#[derive(Clone, Debug, PartialEq)]
pub struct Ready {
    pub version: String,
    relaunch: Vec<OsString>,
    /// Windows: the files are swapped on exit (the helper is already waiting).
    pub on_exit: bool,
}

/// How this copy is installed, if it is one that can update itself.
pub fn current() -> Option<Install> {
    if !crate::update::from_release() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    // the copy an earlier update kept aside does not update itself: its path is the one where
    // the next previous version goes, so the new version would be put there and then thrown
    // away to make room for this one, which would open again (and ask again)
    if inside(&exe, &work_dir()) {
        return None;
    }
    detect(&exe)
}

/// This is the previous version, kept aside by an update (the installed one is elsewhere).
pub fn running_reserve() -> bool {
    std::env::current_exe().is_ok_and(|exe| inside(&exe.canonicalize().unwrap_or(exe), &work_dir()))
}

/// `path` is in `dir` (both as they are on disk).
fn inside(path: &Path, dir: &Path) -> bool {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    path.starts_with(dir)
}

fn detect(exe: &Path) -> Option<Install> {
    if cfg!(target_os = "macos") {
        return match exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")) {
            Some(bundle) => writable(bundle.parent()?).then(|| Install::MacApp(bundle.to_path_buf())),
            None => writable(exe.parent()?).then(|| Install::MacBinary(exe.to_path_buf())),
        };
    }
    if cfg!(target_os = "linux") && exe.starts_with("/usr") {
        return Some(Install::Deb(exe.to_path_buf()));
    }
    let dir = exe.parent()?;
    writable(dir).then(|| Install::Folder(exe.to_path_buf()))
}

/// Can the folder be written? (tried for real)
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".dbcraft-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Can it update without asking the user anything (the .deb wants the password).
pub fn silent(install: &Install) -> bool {
    !matches!(install, Install::Deb(_))
}

/// Folder of the downloaded files (in the config folder).
fn work_dir() -> PathBuf {
    crate::store::config_dir().unwrap_or_else(std::env::temp_dir).join("updates")
}

/// Where the previous version stays aside after an update. A `.noindex` folder: Spotlight does
/// not list what is inside, so looking for DbCraft finds only the installed one.
fn reserve_dir() -> PathBuf {
    work_dir().join("previous.noindex")
}

/// Downloads, checks and installs `release` in place of this copy.
pub fn prepare(install: &Install, release: &Release, phase: &mut dyn FnMut(Phase)) -> Result<Ready> {
    let name = release.asset.clone().ok_or_else(|| format!("la release {} non ha il file per questo sistema", release.version))?;
    let sums_url = release.sums.clone().ok_or_else(|| format!("la release {} non ha le impronte dei file (SHA256SUMS.txt)", release.version))?;
    let sums = crate::update::agent(10, 60)
        .get(&sums_url)
        .call()
        .map_err(|e| format!("impronte: {e}"))?
        .into_string()
        .map_err(|e| format!("impronte: {e}"))?;
    let want = expected_sha256(&sums, &name).ok_or_else(|| format!("{name} non è fra le impronte della release"))?;
    std::fs::create_dir_all(work_dir()).map_err(|e| e.to_string())?;
    let file = work_dir().join(&name);
    download(&release.download, &file, phase)?;
    phase(Phase::Verifying);
    let got = sha256_file(&file)?;
    if got != want {
        let _ = std::fs::remove_file(&file);
        return Err("il file scaricato non corrisponde all'impronta della release: riprova".into());
    }
    phase(Phase::Installing);
    let res = match install {
        Install::MacApp(bundle) => install_mac(&file, bundle, &release.version),
        Install::MacBinary(exe) => install_mac_binary(&file, exe, &release.version),
        Install::Deb(exe) => install_deb(&file, exe),
        Install::Folder(exe) if cfg!(windows) => install_windows(&file, exe),
        Install::Folder(exe) => install_tar(&file, exe),
    };
    let _ = std::fs::remove_file(&file);
    let (relaunch, on_exit) = res?;
    Ok(Ready { version: release.version.clone(), relaunch, on_exit })
}

/// The checksum of `name` in a `SHA256SUMS.txt` (`<hex>  <name>`).
pub fn expected_sha256(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (h, n) = (it.next()?, it.next()?);
        (n.trim_start_matches('*') == name && h.len() == 64).then(|| h.to_ascii_lowercase())
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn download(url: &str, to: &Path, phase: &mut dyn FnMut(Phase)) -> Result<()> {
    let resp = crate::update::agent(10, 60).get(url).call().map_err(|e| format!("download: {e}"))?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut r = resp.into_reader();
    let part = to.with_extension("part");
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 16];
    let (mut done, mut last) = (0u64, 0u64);
    phase(Phase::Downloading { done, total });
    loop {
        let n = r.read(&mut buf).map_err(|e| format!("download: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last > 256 * 1024 {
            last = done;
            phase(Phase::Downloading { done, total });
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    drop(out);
    if total > 0 && done != total {
        return Err(format!("download interrotto ({done} di {total} byte)"));
    }
    std::fs::rename(&part, to).map_err(|e| e.to_string())
}

fn run(cmd: &mut Command, what: &str) -> Result<String> {
    let out = cmd.output().map_err(|e| format!("{what}: non avviato ({e})"))?;
    if !out.status.success() {
        return Err(format!("{what}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// macOS: mounts the DMG, copies the new app next to this one, checks it is the expected
/// version and swaps it with this one (the running app does not notice).
fn install_mac(dmg: &Path, bundle: &Path, version: &str) -> Result<(Vec<OsString>, bool)> {
    let parent = bundle.parent().ok_or("l'app non è in una cartella")?;
    with_dmg(dmg, |src| {
        let staging = parent.join(format!(".dbcraft-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        let staged = staging.join("DbCraft.app");
        // ditto keeps signature and links; without the volume's extended attributes
        run(Command::new("/usr/bin/ditto").args(["--norsrc", "--noextattr", "--noacl"]).arg(src).arg(&staged), "copia della nuova app")?;
        // the expected version and a valid signature (by the same developer as this one): a
        // file put in the release by someone else (even with the right checksums) is not
        // installed
        if let Err(e) = check_app(&staged, version).and_then(|()| check_signature(bundle, &staged)) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
        // the swap: the old one aside, the new one in its place
        let old = staging.join("DbCraft-old.app");
        std::fs::rename(bundle, &old).map_err(|e| format!("spostamento della versione attuale: {e}"))?;
        if let Err(e) = std::fs::rename(&staged, bundle) {
            let _ = std::fs::rename(&old, bundle);
            return Err(format!("sostituzione dell'app: {e}"));
        }
        // the previous version stays aside in the config folder
        // (updates/previous.noindex/DbCraft-previous.app) until another one arrives: if the new
        // one does not start, it can be put back by hand
        let keep = reserve_dir().join("DbCraft-previous.app");
        let _ = std::fs::remove_dir_all(&keep);
        // where versions up to 0.2.7 kept it (Spotlight listed it next to the installed one)
        let _ = std::fs::remove_dir_all(work_dir().join("DbCraft-previous.app"));
        let _ = std::fs::create_dir_all(reserve_dir());
        if std::fs::rename(&old, &keep).is_err() {
            eprintln!("aggiornamento: la versione precedente non resta da parte");
        }
        let _ = std::fs::remove_dir_all(&staging);
        Ok(())
    })?;
    Ok((vec!["/usr/bin/open".into(), bundle.as_os_str().to_owned()], false))
}

/// macOS, program outside an app bundle: the program of the DMG's app in place of this one
/// (the running program keeps its old file until it closes).
fn install_mac_binary(dmg: &Path, exe: &Path, version: &str) -> Result<(Vec<OsString>, bool)> {
    let dir = exe.parent().ok_or("il programma non è in una cartella")?;
    with_dmg(dmg, |src| {
        check_app(src, version)?;
        if run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(src), "firma della nuova app").is_err() {
            return Err("la firma della nuova app non è valida".into());
        }
        let staged = dir.join(format!(".dbcraft-update-{}", std::process::id()));
        std::fs::copy(src.join("Contents/MacOS/dbcraft"), &staged).map_err(|e| format!("copia del nuovo programma: {e}"))?;
        // the previous program stays aside in the config folder (updates/previous.noindex/dbcraft-previous)
        let _ = std::fs::create_dir_all(reserve_dir());
        let _ = std::fs::copy(exe, reserve_dir().join("dbcraft-previous"));
        if let Err(e) = std::fs::rename(&staged, exe) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!("sostituzione del programma: {e}"));
        }
        Ok(())
    })?;
    Ok((vec![exe.as_os_str().to_owned()], false))
}

/// Mounts the DMG (read-only, hidden), calls `f` with its `DbCraft.app` and unmounts it.
fn with_dmg(dmg: &Path, f: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let mount = std::env::temp_dir().join(format!("dbcraft-update-{}", std::process::id()));
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let attach = run(
        Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-noverify", "-mountpoint"]).arg(&mount).arg(dmg),
        "apertura del DMG",
    );
    if let Err(e) = attach {
        let _ = std::fs::remove_dir(&mount);
        return Err(e);
    }
    let src = mount.join("DbCraft.app");
    let result = if src.is_dir() { f(&src) } else { Err("nel DMG non c'è DbCraft.app".into()) };
    let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-quiet"]).arg(&mount).output();
    let _ = std::fs::remove_dir(&mount);
    result
}

/// The app says it is `version` (its Info.plist).
fn check_app(app: &Path, version: &str) -> Result<()> {
    let plist = app.join("Contents/Info.plist");
    let v = run(Command::new("/usr/libexec/PlistBuddy").args(["-c", "Print :CFBundleShortVersionString"]).arg(&plist), "versione della nuova app")?;
    if v.trim() != version {
        return Err(format!("la nuova app dice di essere la {}, non la {version}", v.trim()));
    }
    Ok(())
}

/// The developer who signed an app (codesign's `TeamIdentifier`); `None` for an ad-hoc
/// signature.
fn team_id(app: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(app).output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t != "not set" && t.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// The new app is properly signed, and by the same developer as this one.
fn check_signature(current: &Path, new: &Path) -> Result<()> {
    if run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(new), "firma della nuova app").is_err() {
        return Err("la firma della nuova app non è valida".into());
    }
    if let Some(team) = team_id(current) {
        let req = format!("=anchor apple generic and certificate leaf[subject.OU] = \"{team}\"");
        if run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict", "-R"]).arg(&req).arg(new), "sviluppatore della nuova app").is_err() {
            return Err(format!("la nuova app non è firmata dallo stesso sviluppatore ({team}): non la installo"));
        }
    }
    Ok(())
}

/// Linux .deb: the package manager, with the password asked by the system.
fn install_deb(deb: &Path, exe: &Path) -> Result<(Vec<OsString>, bool)> {
    let deb = deb.canonicalize().map_err(|e| e.to_string())?;
    run(Command::new("pkexec").arg("dpkg").arg("-i").arg(&deb), "installazione del pacchetto")
        .map_err(|e| format!("{e}\n(serve pkexec per chiedere la password; altrimenti: sudo apt install {})", deb.display()))?;
    Ok((vec![exe.as_os_str().to_owned()], false))
}

/// Linux .tar.gz: the new files in place of the folder's ones.
fn install_tar(tgz: &Path, exe: &Path) -> Result<(Vec<OsString>, bool)> {
    install_tar_in(tgz, exe, &work_dir())
}

fn install_tar_in(tgz: &Path, exe: &Path, work: &Path) -> Result<(Vec<OsString>, bool)> {
    let dir = exe.parent().ok_or("il programma non è in una cartella")?;
    let tmp = work.join(format!("extracted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    run(Command::new("tar").arg("-xzf").arg(tgz).arg("-C").arg(&tmp), "apertura dell'archivio")?;
    // the archive has a single folder: dbcraft-<version>-linux-x86_64/
    let top = std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.is_dir()).ok_or("archivio vuoto")?;
    if !top.join("dbcraft").is_file() {
        return Err("nell'archivio non c'è il programma".into());
    }
    for e in std::fs::read_dir(&top).map_err(|e| e.to_string())?.flatten() {
        let src = e.path();
        let dst = dir.join(e.file_name());
        // copied alongside and renamed: the running program stays valid
        let staged = dir.join(format!(".{}.new", e.file_name().to_string_lossy()));
        std::fs::copy(&src, &staged).map_err(|e| e.to_string())?;
        std::fs::rename(&staged, &dst).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_dir_all(&tmp);
    Ok((vec![exe.as_os_str().to_owned()], false))
}

/// Windows: the running program cannot be replaced; a PowerShell helper waits for the app to
/// close, copies the files and reopens it.
fn install_windows(zip: &Path, exe: &Path) -> Result<(Vec<OsString>, bool)> {
    let dir = exe.parent().ok_or("il programma non è in una cartella")?;
    // the downloaded file stays until the helper has used it
    let keep = work_dir().join("update.zip");
    std::fs::copy(zip, &keep).map_err(|e| e.to_string())?;
    let keep = keep.canonicalize().map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join("dbcraft-update");
    let q = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
    let script = format!(
        "$p = {pid}; while (Get-Process -Id $p -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 200 }}; \
         $t = {tmp}; Remove-Item -LiteralPath $t -Recurse -Force -ErrorAction SilentlyContinue; \
         Expand-Archive -LiteralPath {zip} -DestinationPath $t -Force; \
         $top = Get-ChildItem -LiteralPath $t | Select-Object -First 1; \
         Copy-Item -Path (Join-Path $top.FullName '*') -Destination {dir} -Recurse -Force; \
         Remove-Item -LiteralPath $t -Recurse -Force -ErrorAction SilentlyContinue; Remove-Item -LiteralPath {zip} -ErrorAction SilentlyContinue; \
         Start-Process -FilePath {exe}",
        pid = std::process::id(),
        tmp = q(&tmp),
        zip = q(&keep),
        dir = q(dir),
        exe = q(exe),
    );
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window (DbCraft is a GUI app)
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().map_err(|e| format!("aiutante dell'aggiornamento: {e}"))?;
    Ok((Vec::new(), true))
}

/// Opens a web page in the default browser.
pub fn open_url(url: &str) {
    let r = if cfg!(target_os = "macos") {
        Command::new("/usr/bin/open").arg(url).spawn()
    } else if cfg!(windows) {
        Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
    if let Err(e) = r {
        eprintln!("open {url}: {e}");
    }
}

/// Reopens the app as soon as this one closes (an `sh` that waits and then starts it).
/// Afterwards the app must close.
pub fn relaunch_after_exit(ready: &Ready) -> Result<()> {
    if ready.on_exit || ready.relaunch.is_empty() {
        return Ok(());
    }
    if cfg!(unix) {
        Command::new("/bin/sh")
            .arg("-c")
            .arg(r#"pid=$1; shift; i=0; while kill -0 "$pid" 2>/dev/null && [ $i -lt 600 ]; do sleep 0.1; i=$((i+1)); done; exec "$@""#)
            .arg("sh")
            .arg(std::process::id().to_string())
            .args(&ready.relaunch)
            .env_remove("DBCRAFT_PRETEND_VERSION")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("riavvio: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_kept_aside_is_recognised() {
        // as on disk (macOS: /var is /private/var), like the program's path
        let tmp = std::env::temp_dir();
        let base = tmp.canonicalize().unwrap_or(tmp).join(format!("dbcraft-reserve-{}", std::process::id()));
        let work = base.join("updates");
        let keep = work.join("previous.noindex/DbCraft-previous.app/Contents/MacOS");
        std::fs::create_dir_all(&keep).unwrap();
        std::fs::create_dir_all(base.join("Applications/DbCraft.app/Contents/MacOS")).unwrap();
        assert!(inside(&keep.join("dbcraft"), &work));
        // the old place too (updates/DbCraft-previous.app)
        assert!(inside(&work.join("DbCraft-previous.app/Contents/MacOS/dbcraft"), &work));
        let installed = base.join("Applications/DbCraft.app/Contents/MacOS/dbcraft");
        assert!(!inside(&installed, &work));
        // not fooled by a name that only begins the same way
        assert!(!inside(&base.join("updates-other/dbcraft"), &work));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn checksums() {
        let s = "3a1b…\nabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd  DbCraft-0.2.4-macos-arm64.dmg\n\
                 0000000000000000000000000000000000000000000000000000000000000000 *dbcraft_0.2.4_amd64.deb\n";
        assert_eq!(expected_sha256(s, "DbCraft-0.2.4-macos-arm64.dmg").map(|h| h.len()), Some(64));
        assert_eq!(expected_sha256(s, "dbcraft_0.2.4_amd64.deb"), Some("0".repeat(64)));
        assert!(expected_sha256(s, "other").is_none());
    }

    #[test]
    fn linux_archive_replaces_files() {
        if !cfg!(unix) {
            return;
        }
        let base = std::env::temp_dir().join(format!("dbcraft-selfupdate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let inst = base.join("installed");
        std::fs::create_dir_all(&inst).unwrap();
        std::fs::write(inst.join("dbcraft"), b"old").unwrap();
        std::fs::write(inst.join("README.md"), b"old").unwrap();
        // the release archive: one folder with the program
        let src = base.join("src/dbcraft-9.9.9-linux-x86_64");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("dbcraft"), b"new").unwrap();
        std::fs::write(src.join("README.md"), b"new").unwrap();
        let tgz = base.join("rel.tar.gz");
        assert!(Command::new("tar").arg("-czf").arg(&tgz).arg("-C").arg(base.join("src")).arg("dbcraft-9.9.9-linux-x86_64").status().unwrap().success());
        let (relaunch, on_exit) = install_tar_in(&tgz, &inst.join("dbcraft"), &base.join("updates")).unwrap();
        assert!(!on_exit && relaunch[0] == inst.join("dbcraft").as_os_str());
        assert_eq!(std::fs::read(inst.join("dbcraft")).unwrap(), b"new");
        assert_eq!(std::fs::read(inst.join("README.md")).unwrap(), b"new");
        std::fs::remove_dir_all(&base).unwrap();
    }
}
