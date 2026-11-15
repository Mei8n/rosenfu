//! Fixed-release road data transport. Never extracts ZIP paths to the filesystem.
//! Call read from a blocking worker; cancel_all is independent of the read mutex.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{Cursor, Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
const FILE_MAX: usize = 5 * 1024 * 1024;
const ZIP_MAX: usize = 64 * 1024 * 1024;
const EXPANDED_MAX: usize = 256 * 1024 * 1024;
const MEMORY_MAX: usize = 96 * 1024 * 1024;
const CACHE_MAX: u64 = 512 * 1024 * 1024;
const PREFIX: &str = "https://github.com/Mei8n/rosenfu/releases/download/";

pub struct Release {
    pub tag: String,
    pub catalog_sha256: String,
}
#[derive(Clone, Deserialize, PartialEq, Eq)]
struct Asset {
    name: String,
    manifest: String,
    manifest_sha256: String,
    sha256: String,
    bytes: usize,
    expanded_bytes: usize,
    files: usize,
}
#[derive(Clone, Deserialize)]
struct Catalog {
    format: String,
    version: u32,
    publishable: bool,
    index_sha256: String,
    assets: Vec<Asset>,
}
#[derive(Default)]
struct State {
    catalog: Option<Catalog>,
    index: Option<Vec<u8>>,
    archives: VecDeque<VerifiedArchive>,
    #[cfg(test)]
    validation_count: usize,
}
struct VerifiedArchive {
    asset: Asset,
    archive: zip::ZipArchive<Cursor<Vec<u8>>>,
    entries: HashMap<String, (usize, String)>,
}
impl VerifiedArchive {
    fn read(&mut self, path: &str, check: &impl Fn() -> Result<()>) -> Result<Vec<u8>> {
        check()?;
        let (size, sha) = self
            .entries
            .get(path)
            .ok_or("道路cell未提供（道路なしを意味しません）")?;
        let mut file = self
            .archive
            .by_name(path)
            .map_err(|_| "道路ZIP entry欠落")?;
        let content = read_bounded(&mut file, *size, check)?;
        ensure(
            content.len() == *size && hash(&content) == *sha,
            "道路ZIP entry hash不一致",
        )?;
        check()?;
        Ok(content)
    }
}
pub struct RoadDelivery {
    root: PathBuf,
    release: Option<Release>,
    client: reqwest::blocking::Client,
    epoch: AtomicU64,
    state: Mutex<State>,
}
type Result<T> = std::result::Result<T, String>;
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}
fn number(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && s.len() <= 6 && s.bytes().all(|b| b.is_ascii_digit())
}
fn grid(s: &str) -> bool {
    let p: Vec<_> = s.split('_').collect();
    p.len() == 2 && number(p[0]) && number(p[1])
}
fn safe_path(s: &str) -> bool {
    if s == "index.json" {
        return true;
    }
    let p: Vec<_> = s.split('/').collect();
    p.len() == 4
        && p[0] == "packs"
        && !p[1].is_empty()
        && p[1]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && grid(p[2])
        && (p[3] == "manifest.json" || p[3].strip_suffix(".json").is_some_and(grid))
}
fn allowed_redirect(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && match url.host_str() {
            Some("github.com") => url.path().starts_with("/Mei8n/rosenfu/releases/download/"),
            Some("release-assets.githubusercontent.com") => true,
            _ => false,
        }
}
fn read_bounded(
    reader: &mut impl Read,
    max: usize,
    check: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0; 32768];
    loop {
        check()?;
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("道路データ読込失敗: {e}"))?;
        if n == 0 {
            break;
        }
        ensure(output.len().saturating_add(n) <= max, "道路データ容量超過")?;
        output.extend_from_slice(&chunk[..n]);
    }
    check()?;
    Ok(output)
}
impl RoadDelivery {
    pub fn new(root: PathBuf, release: Option<Release>) -> Result<Self> {
        if let Some(r) = &release {
            ensure(
                r.tag.starts_with("data-road-")
                    && r.tag.len() <= 100
                    && r.tag
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    && valid_hash(&r.catalog_sha256),
                "道路配信設定不正",
            )?;
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("Rosenfu/0.1 road-data")
            .referer(false)
            .redirect(reqwest::redirect::Policy::custom(|a| {
                if a.previous().len() < 4 && allowed_redirect(a.url()) {
                    a.follow()
                } else {
                    a.error("road redirect forbidden")
                }
            }))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            release,
            client,
            epoch: AtomicU64::new(0),
            state: Mutex::new(State::default()),
        })
    }
    pub fn cancel_all(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        self.read_with(path, &|url, max, check| {
            let mut response = self
                .client
                .get(url)
                .send()
                .map_err(|e| format!("道路データ取得失敗: {e}"))?;
            ensure(
                response.status() == reqwest::StatusCode::OK && allowed_redirect(response.url()),
                "道路データ取得不可（道路なしを意味しません）",
            )?;
            ensure(
                response.content_length().is_none_or(|n| n <= max as u64),
                "道路データ容量超過",
            )?;
            read_bounded(&mut response, max, &|| check())
        })
    }
    fn read_with(
        &self,
        path: &str,
        fetch: &impl Fn(&str, usize, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        ensure(safe_path(path), "道路データpath不正")?;
        let release = self.release.as_ref().ok_or("全国道路配信は未設定です")?;
        let epoch = self.epoch.load(Ordering::SeqCst);
        let check = || {
            ensure(
                self.epoch.load(Ordering::SeqCst) == epoch,
                "道路データ取得を取り消しました",
            )
        };
        let mut state = self.state.lock().map_err(|_| "道路cache lock失敗")?;
        check()?;
        self.prepare_root()?;
        if state.catalog.is_none() {
            let bytes = self.blob(
                "catalog.json",
                &release.catalog_sha256,
                FILE_MAX,
                &check,
                fetch,
            )?;
            let catalog: Catalog = serde_json::from_slice(&bytes).map_err(|_| "道路catalog不正")?;
            validate_catalog(&catalog)?;
            state.catalog = Some(catalog);
        }
        let catalog = state.catalog.as_ref().ok_or("道路catalogなし")?;
        if state.index.is_none() {
            let bytes = self.blob("index.json", &catalog.index_sha256, FILE_MAX, &check, fetch)?;
            validate_index(&bytes, catalog)?;
            state.index = Some(bytes);
        }
        if path == "index.json" {
            check()?;
            return Ok(state.index.clone().ok_or("道路indexなし")?);
        }
        let catalog = state.catalog.as_ref().ok_or("道路catalogなし")?;
        let prefix = path.rsplit_once('/').ok_or("道路path不正")?.0;
        let asset = catalog
            .assets
            .iter()
            .find(|a| a.manifest == format!("{prefix}/manifest.json"))
            .ok_or("道路未提供範囲です")?
            .clone();
        if let Some(position) = state.archives.iter().position(|a| a.asset == asset) {
            let mut cached = state
                .archives
                .remove(position)
                .ok_or("道路memory cache不正")?;
            let result = cached.read(path, &check);
            // Cancellation is not corruption; immutable bytes remain valid.
            if result.is_ok() || check().is_err() {
                state.archives.push_back(cached);
            }
            return result;
        }
        let bytes = self.blob(&asset.name, &asset.sha256, asset.bytes, &check, fetch)?;
        let mut verified = verify_archive(bytes, &asset, &check)?;
        #[cfg(test)]
        {
            state.validation_count += 1;
        }
        let result = verified.read(path, &check)?;
        keep_archive(&mut state, verified, MEMORY_MAX)?;
        Ok(result)
    }
    fn prepare_root(&self) -> Result<()> {
        // App-owned cache directory, never a path supplied by the webview.
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        ensure(
            !fs::symlink_metadata(&self.root)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "道路cache symlink不正",
        )
    }
    fn blob(
        &self,
        name: &str,
        sha: &str,
        max: usize,
        check: &impl Fn() -> Result<()>,
        fetch: &impl Fn(&str, usize, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        let dest = self.root.join(format!("{sha}.data"));
        if let Ok(meta) = fs::symlink_metadata(&dest) {
            ensure(
                meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= max as u64,
                "道路cache不正",
            )?;
            let bytes = read_bounded(
                &mut fs::File::open(&dest).map_err(|e| e.to_string())?,
                max,
                check,
            )?;
            ensure(hash(&bytes) == sha, "道路cache hash不一致")?;
            return Ok(bytes);
        }
        let release = self.release.as_ref().ok_or("道路配信未設定")?;
        check()?;
        let bytes = fetch(&format!("{PREFIX}{}/{name}", release.tag), max, check)?;
        check()?;
        ensure(
            bytes.len() <= max && hash(&bytes) == sha,
            "道路配信hash/size不一致",
        )?;
        self.evict(bytes.len() as u64)?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        temp.write_all(&bytes).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        check()?;
        // Another instance may have published this hash. Never replace a different file.
        match temp.persist_noclobber(&dest) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(&dest).map_err(|e| e.to_string())?;
                ensure(
                    meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= max as u64,
                    "道路cache競合type不正",
                )?;
                let existing = read_bounded(
                    &mut fs::File::open(&dest).map_err(|e| e.to_string())?,
                    max,
                    check,
                )?;
                ensure(hash(&existing) == sha, "道路cache競合")?;
            }
            Err(e) => return Err(e.error.to_string()),
        }
        Ok(bytes)
    }
    fn evict(&self, needed: u64) -> Result<()> {
        let mut entries = Vec::new();
        let mut total = 0u64;
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.strip_suffix(".data").is_some_and(valid_hash) {
                continue;
            }
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            ensure(
                meta.is_file() && !meta.file_type().is_symlink(),
                "道路cache不正",
            )?;
            total = total.saturating_add(meta.len());
            entries.push((meta.modified().ok(), entry.path(), meta.len()));
        }
        entries.sort_by_key(|a| a.0);
        for (_, path, size) in entries {
            if total.saturating_add(needed) <= CACHE_MAX {
                break;
            }
            fs::remove_file(path).map_err(|e| e.to_string())?;
            total = total.saturating_sub(size);
        }
        ensure(
            total.saturating_add(needed) <= CACHE_MAX,
            "道路cache容量超過",
        )
    }
}
fn keep_archive(state: &mut State, archive: VerifiedArchive, limit: usize) -> Result<()> {
    ensure(archive.asset.bytes <= limit, "道路memory容量超過")?;
    while state.archives.len() >= 4
        || state.archives.iter().map(|a| a.asset.bytes).sum::<usize>() + archive.asset.bytes > limit
    {
        state.archives.pop_front().ok_or("道路memory容量超過")?;
    }
    state.archives.push_back(archive);
    Ok(())
}

fn validate_catalog(c: &Catalog) -> Result<()> {
    ensure(
        c.format == "rosenfu-road-release"
            && c.version == 1
            && c.publishable
            && valid_hash(&c.index_sha256)
            && !c.assets.is_empty()
            && c.assets.len() <= 998,
        "道路catalog契約不一致",
    )?;
    let mut names = std::collections::HashSet::new();
    let mut paths = std::collections::HashSet::new();
    for a in &c.assets {
        ensure(
            safe_path(&a.manifest)
                && a.manifest.ends_with("/manifest.json")
                && a.name.ends_with(".zip")
                && a.name.len() <= 100
                && a.name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && names.insert(&a.name)
                && paths.insert(&a.manifest)
                && valid_hash(&a.sha256)
                && valid_hash(&a.manifest_sha256)
                && a.bytes > 0
                && a.bytes <= ZIP_MAX
                && a.expanded_bytes <= EXPANDED_MAX
                && a.files > 0
                && a.files <= 2501,
            "道路asset不正",
        )?;
    }
    Ok(())
}
fn validate_index(bytes: &[u8], c: &Catalog) -> Result<()> {
    let index: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| "道路index不正")?;
    ensure(
        index["format"] == "rosenfu-road-pack-index" && index["version"] == 1,
        "道路index形式不正",
    )?;
    let regions = index["regions"].as_array().ok_or("道路regions不正")?;
    let mut seen = std::collections::HashSet::new();
    for r in regions {
        for s in r["shards"].as_array().ok_or("道路shards不正")? {
            let a = c
                .assets
                .iter()
                .find(|a| s["manifest"] == a.manifest)
                .ok_or("道路catalog/index不一致")?;
            ensure(s["sha256"] == a.manifest_sha256, "道路manifest hash不一致")?;
            ensure(seen.insert(a.manifest.as_str()), "道路index重複shard")?;
        }
    }
    ensure(seen.len() == c.assets.len(), "道路asset数不一致")
}
fn verify_archive(
    bytes: Vec<u8>,
    asset: &Asset,
    check: &impl Fn() -> Result<()>,
) -> Result<VerifiedArchive> {
    ensure(
        bytes.len() == asset.bytes && hash(&bytes) == asset.sha256,
        "道路ZIP hash不一致",
    )?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "道路ZIP形式不正")?;
    ensure(archive.len() == asset.files, "道路ZIP file数不一致")?;
    let manifest = read_bounded(
        &mut archive
            .by_name(&asset.manifest)
            .map_err(|_| "道路manifest欠落")?,
        FILE_MAX,
        check,
    )?;
    ensure(
        hash(&manifest) == asset.manifest_sha256,
        "道路manifest hash不一致",
    )?;
    let value: serde_json::Value =
        serde_json::from_slice(&manifest).map_err(|_| "道路manifest形式不正")?;
    let cells = value["cells"].as_object().ok_or("道路cells不正")?;
    ensure(cells.len() + 1 == asset.files, "道路cell数不一致")?;
    let prefix = asset
        .manifest
        .rsplit_once('/')
        .ok_or("道路manifest path不正")?
        .0;
    let mut expected = HashMap::new();
    expected.insert(
        asset.manifest.clone(),
        (manifest.len(), asset.manifest_sha256.clone()),
    );
    for (key, c) in cells {
        ensure(
            grid(key) && c["file"] == format!("{key}.json"),
            "道路cell path不正",
        )?;
        let size = c["bytes"].as_u64().ok_or("道路cell size不正")?;
        let sha = c["sha256"].as_str().ok_or("道路cell hash不正")?;
        ensure(
            size <= FILE_MAX as u64 && valid_hash(sha),
            "道路cell制限超過",
        )?;
        expected.insert(
            format!("{prefix}/{key}.json"),
            (size as usize, sha.to_owned()),
        );
    }
    let entries = expected.clone();
    let mut total = 0usize;
    for i in 0..archive.len() {
        check()?;
        let mut file = archive.by_index(i).map_err(|_| "道路ZIP entry不正")?;
        let name = file.name().to_owned();
        ensure(
            safe_path(&name)
                && file.enclosed_name().is_some()
                && file
                    .unix_mode()
                    .is_none_or(|m| m & 0o170000 == 0 || m & 0o170000 == 0o100000),
            "道路ZIP危険なpath/type",
        )?;
        let (size, sha) = expected.remove(&name).ok_or("道路ZIP重複/想定外entry")?;
        ensure(file.size() == size as u64, "道路ZIP展開size不一致")?;
        total = total.saturating_add(size);
        ensure(
            total <= asset.expanded_bytes && total <= EXPANDED_MAX,
            "道路ZIP展開容量超過",
        )?;
        let content = read_bounded(&mut file, size, check)?;
        ensure(
            content.len() == size && hash(&content) == sha,
            "道路ZIP entry hash不一致",
        )?;
    }
    check()?;
    ensure(
        expected.is_empty() && total == asset.expanded_bytes,
        "道路ZIP内容不足",
    )?;
    Ok(VerifiedArchive {
        asset: asset.clone(),
        archive,
        entries,
    })
}

#[cfg(test)]
fn validate_zip(
    bytes: &[u8],
    asset: &Asset,
    wanted: &str,
    check: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    verify_archive(bytes.to_vec(), asset, check)?.read(wanted, check)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use zip::write::SimpleFileOptions;
    fn fixture(extra: Option<&str>) -> (Asset, Vec<u8>, Vec<u8>, Vec<u8>) {
        let cell = b"{\"ways\":[]}";
        let manifest=serde_json::to_vec(&json!({"cells":{"6500_1500":{"file":"6500_1500.json","bytes":cell.len(),"sha256":hash(cell)}}})).unwrap();
        let path = "packs/test/130_30/manifest.json";
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file(path, opts).unwrap();
        z.write_all(&manifest).unwrap();
        z.start_file("packs/test/130_30/6500_1500.json", opts)
            .unwrap();
        z.write_all(cell).unwrap();
        if let Some(name) = extra {
            z.start_file(name, opts).unwrap();
            z.write_all(b"bad").unwrap();
        }
        let bytes = z.finish().unwrap().into_inner();
        let a = Asset {
            name: "test-130_30.zip".into(),
            manifest: path.into(),
            manifest_sha256: hash(&manifest),
            sha256: hash(&bytes),
            bytes: bytes.len(),
            expanded_bytes: manifest.len() + cell.len(),
            files: 2,
        };
        (a, bytes, manifest, cell.to_vec())
    }
    fn server_fixture() -> (Release, HashMap<String, Vec<u8>>) {
        let (a, zip, _, _) = fixture(None);
        let index=serde_json::to_vec(&json!({"format":"rosenfu-road-pack-index","version":1,"coverage_verified":false,"regions":[{"shards":[{"manifest":a.manifest,"sha256":a.manifest_sha256}]}]})).unwrap();
        let catalog=serde_json::to_vec(&json!({"format":"rosenfu-road-release","version":1,"publishable":true,"coverage_verified":false,"index_sha256":hash(&index),"assets":[{"name":a.name,"manifest":a.manifest,"manifest_sha256":a.manifest_sha256,"sha256":a.sha256,"bytes":a.bytes,"expanded_bytes":a.expanded_bytes,"files":a.files}]})).unwrap();
        (
            Release {
                tag: "data-road-test".into(),
                catalog_sha256: hash(&catalog),
            },
            HashMap::from([
                ("catalog.json".into(), catalog),
                ("index.json".into(), index),
                (a.name, zip),
            ]),
        )
    }
    #[test]
    fn valid_zip_cell_and_manifest() {
        let (a, b, m, c) = fixture(None);
        assert_eq!(
            validate_zip(&b, &a, "packs/test/130_30/6500_1500.json", &|| Ok(())).unwrap(),
            c
        );
        assert_eq!(validate_zip(&b, &a, &a.manifest, &|| Ok(())).unwrap(), m);
    }
    #[test]
    fn traversal_and_unexpected_entries_fail() {
        for name in ["../evil", "/absolute", "packs/test/130_30/other.json"] {
            let (mut a, b, _, _) = fixture(Some(name));
            a.files = 3;
            assert!(validate_zip(&b, &a, &a.manifest, &|| Ok(())).is_err());
        }
    }
    #[test]
    fn bad_hash_fails() {
        let (mut a, b, _, _) = fixture(None);
        a.sha256 = "0".repeat(64);
        assert!(validate_zip(&b, &a, &a.manifest, &|| Ok(())).is_err());
    }
    #[test]
    fn expanded_size_limit() {
        let (mut a, b, _, _) = fixture(None);
        a.expanded_bytes = 1;
        assert!(validate_zip(&b, &a, &a.manifest, &|| Ok(())).is_err());
    }
    #[test]
    fn cancellation_during_zip() {
        let (a, b, _, _) = fixture(None);
        assert!(validate_zip(&b, &a, &a.manifest, &|| Err("cancel".into())).is_err());
    }
    #[test]
    fn missing_cell_never_empty() {
        let (a, b, _, _) = fixture(None);
        assert!(validate_zip(&b, &a, "packs/test/130_30/6500_1501.json", &|| Ok(())).is_err());
    }
    #[test]
    fn request_paths() {
        for bad in [
            "/index.json",
            "packs/../a/b",
            "packs/test/130_30/../../x",
            "packs/test/130_30/1_2.json?x",
            "packs\\test\\130_30\\1_2.json",
        ] {
            assert!(!safe_path(bad));
        }
        assert!(safe_path("packs/test/130_30/6500_1500.json"));
    }
    #[test]
    fn redirect_allowlist() {
        for url in [
            "http://github.com/Mei8n/rosenfu/releases/download/x/y",
            "https://evil.test/x",
            "https://github.com/other/repo/x",
            "https://release-assets.githubusercontent.com.evil.test/x",
            "https://user@release-assets.githubusercontent.com/x",
            "https://release-assets.githubusercontent.com:444/x",
        ] {
            assert!(!allowed_redirect(&reqwest::Url::parse(url).unwrap()));
        }
        assert!(allowed_redirect(
            &reqwest::Url::parse("https://release-assets.githubusercontent.com/x?signature=abc")
                .unwrap()
        ));
    }
    #[test]
    fn symlink_zip_entry_rejected() {
        let (mut a, _, manifest, cell) = fixture(None);
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        z.start_file(&a.manifest, options).unwrap();
        z.write_all(&manifest).unwrap();
        z.add_symlink(
            "packs/test/130_30/6500_1500.json",
            std::str::from_utf8(&cell).unwrap(),
            options,
        )
        .unwrap();
        let bytes = z.finish().unwrap().into_inner();
        a.bytes = bytes.len();
        a.sha256 = hash(&bytes);
        assert!(validate_zip(&bytes, &a, &a.manifest, &|| Ok(())).is_err());
    }
    #[test]
    fn cache_budget_rejects_oversized_publication() {
        let t = tempfile::tempdir().unwrap();
        let d = RoadDelivery::new(t.path().to_owned(), None).unwrap();
        assert!(d.evict(CACHE_MAX + 1).is_err());
    }
    #[test]
    fn bounded_reader() {
        assert!(read_bounded(&mut Cursor::new(vec![0; 100]), 10, &|| Ok(())).is_err());
    }
    #[test]
    fn unset_release_fails_without_fetch() {
        let t = tempfile::tempdir().unwrap();
        let d = RoadDelivery::new(t.path().join("cache"), None).unwrap();
        assert!(d
            .read_with("index.json", &|_, _, _| panic!("must not fetch"))
            .is_err());
    }
    #[test]
    fn cache_reuse_offline_and_unverified_preserved() {
        let t = tempfile::tempdir().unwrap();
        let (r, data) = server_fixture();
        let sha = r.catalog_sha256.clone();
        let d = RoadDelivery::new(t.path().join("cache"), Some(r)).unwrap();
        let result = d
            .read_with("packs/test/130_30/6500_1500.json", &|url, _, _| {
                Ok(data[url.rsplit('/').next().unwrap()].clone())
            })
            .unwrap();
        assert_eq!(result, b"{\"ways\":[]}");
        let offline = RoadDelivery::new(
            t.path().join("cache"),
            Some(Release {
                tag: "data-road-test".into(),
                catalog_sha256: sha,
            }),
        )
        .unwrap();
        let index = offline
            .read_with("index.json", &|_, _, _| Err("offline".into()))
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&index).unwrap()["coverage_verified"],
            false
        );
        assert!(offline
            .read_with("packs/test/130_30/6500_1500.json", &|_, _, _| Err(
                "offline".into()
            ))
            .is_ok());
    }
    #[test]
    fn memory_archive_budget_evicts_oldest() {
        let (a, b, _, _) = fixture(None);
        let mut state = State::default();
        keep_archive(
            &mut state,
            verify_archive(b.clone(), &a, &|| Ok(())).unwrap(),
            a.bytes,
        )
        .unwrap();
        keep_archive(
            &mut state,
            verify_archive(b.clone(), &a, &|| Ok(())).unwrap(),
            a.bytes,
        )
        .unwrap();
        assert_eq!(state.archives.len(), 1);
        assert!(keep_archive(&mut state, verify_archive(b, &a, &|| Ok(())).unwrap(), 1).is_err());
        assert_eq!(state.archives.len(), 1);
        let (_, b, _, _) = fixture(None);
        for _ in 0..8 {
            keep_archive(
                &mut state,
                verify_archive(b.clone(), &a, &|| Ok(())).unwrap(),
                MEMORY_MAX,
            )
            .unwrap();
        }
        assert_eq!(state.archives.len(), 4);
    }
    #[test]
    fn repeated_cells_validate_whole_archive_only_once() {
        let t = tempfile::tempdir().unwrap();
        let (r, data) = server_fixture();
        let d = RoadDelivery::new(t.path().join("cache"), Some(r)).unwrap();
        let fetch = |url: &str, _: usize, _: &dyn Fn() -> Result<()>| {
            Ok(data[url.rsplit('/').next().unwrap()].clone())
        };
        let path = "packs/test/130_30/6500_1500.json";
        for _ in 0..12 {
            assert_eq!(d.read_with(path, &fetch).unwrap(), b"{\"ways\":[]}");
        }
        {
            let state = d.state.lock().unwrap();
            assert_eq!(state.validation_count, 1);
            assert_eq!(state.archives.len(), 1);
        }
        d.state.lock().unwrap().archives.clear();
        assert!(d.read_with(path, &fetch).is_ok());
        assert_eq!(d.state.lock().unwrap().validation_count, 2);
    }
    #[test]
    fn disk_corruption_cannot_change_verified_memory_and_is_rejected_on_reload() {
        let t = tempfile::tempdir().unwrap();
        let (r, data) = server_fixture();
        let d = RoadDelivery::new(t.path().join("cache"), Some(r)).unwrap();
        let fetch = |url: &str, _: usize, _: &dyn Fn() -> Result<()>| {
            Ok(data[url.rsplit('/').next().unwrap()].clone())
        };
        let path = "packs/test/130_30/6500_1500.json";
        let good = d.read_with(path, &fetch).unwrap();
        let sha = d.state.lock().unwrap().archives[0].asset.sha256.clone();
        fs::write(d.root.join(format!("{sha}.data")), b"corrupt").unwrap();
        assert_eq!(d.read_with(path, &fetch).unwrap(), good);
        d.state.lock().unwrap().archives.clear();
        assert!(d.read_with(path, &fetch).is_err());
    }
    #[test]
    fn failure_does_not_publish_cache() {
        let t = tempfile::tempdir().unwrap();
        let (r, _) = server_fixture();
        let d = RoadDelivery::new(t.path().join("cache"), Some(r)).unwrap();
        assert!(d
            .read_with("index.json", &|_, _, _| Err("network failure".into()))
            .is_err());
        assert_eq!(fs::read_dir(t.path().join("cache")).unwrap().count(), 0);
    }
    #[test]
    fn cancellation_does_not_publish_and_next_request_recovers() {
        let t = tempfile::tempdir().unwrap();
        let (r, data) = server_fixture();
        let d = RoadDelivery::new(t.path().join("cache"), Some(r)).unwrap();
        assert!(d
            .read_with("index.json", &|url, _, _| {
                d.cancel_all();
                Ok(data[url.rsplit('/').next().unwrap()].clone())
            })
            .is_err());
        assert_eq!(fs::read_dir(t.path().join("cache")).unwrap().count(), 0);
        assert!(d
            .read_with("index.json", &|url, _, _| Ok(data
                [url.rsplit('/').next().unwrap()]
            .clone()))
            .is_ok());
    }
    #[test]
    fn corrupt_cache_rejected() {
        let t = tempfile::tempdir().unwrap();
        let (r, _) = server_fixture();
        let cache = t.path().join("cache");
        fs::create_dir(&cache).unwrap();
        fs::write(cache.join(format!("{}.data", r.catalog_sha256)), b"corrupt").unwrap();
        let d = RoadDelivery::new(cache, Some(r)).unwrap();
        assert!(d
            .read_with("index.json", &|_, _, _| panic!(
                "cache corruption must not hide"
            ))
            .is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_cache_rejected() {
        let t = tempfile::tempdir().unwrap();
        let (r, _) = server_fixture();
        let cache = t.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let target = t.path().join("other");
        fs::write(&target, b"secret").unwrap();
        std::os::unix::fs::symlink(&target, cache.join(format!("{}.data", r.catalog_sha256)))
            .unwrap();
        let d = RoadDelivery::new(cache, Some(r)).unwrap();
        assert!(d
            .read_with("index.json", &|_, _, _| panic!("no fetch"))
            .is_err());
        assert_eq!(fs::read(target).unwrap(), b"secret");
    }
}
