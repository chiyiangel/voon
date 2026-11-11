//! Local thumbnails belong to the index, independently of decoded-image LRU
//! residency. Source reads, decoding and asset writes run in bounded workers.

use super::{AppState, grid::CoverStatus};
use gpui::{AppContext, Context, RenderImage};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use voon_core::Cover;

const MEMORY_CAPACITY: usize = 256;
const CONCURRENCY: usize = 4;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThumbnailState {
    Loading,
    Ready,
    Failed(String),
}

struct Entry {
    identity: u64,
    used: u64,
    state: ThumbnailState,
    path: Option<PathBuf>,
    image: Option<Arc<RenderImage>>,
    started: bool,
}

pub(super) struct CoverThumbnails {
    directory: Option<PathBuf>,
    entries: HashMap<Cover, Entry>,
    queue: VecDeque<Cover>,
    active: HashSet<u64>,
    clock: u64,
    // Invalidation and the two atomic publication renames share this gate.
    // Slow reads/decoding/writes never hold it.
    generation: Arc<Mutex<u64>>,
}

impl CoverThumbnails {
    pub(super) fn new(database: Option<&Path>) -> Self {
        Self {
            directory: database.map(|path| {
                let mut local = path.as_os_str().to_owned();
                local.push(".covers-v1");
                local.into()
            }),
            entries: HashMap::new(),
            queue: VecDeque::new(),
            active: HashSet::new(),
            clock: 0,
            generation: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) fn invalidate(&mut self) {
        *self.generation.lock().unwrap() += 1;
        self.entries.clear();
        self.queue.clear();
    }
}

impl Drop for CoverThumbnails {
    fn drop(&mut self) {
        *self.generation.lock().unwrap() += 1;
    }
}

impl AppState {
    pub fn cover_thumbnail_directory(&self) -> Option<&Path> {
        self.thumbnails.directory.as_deref()
    }

    pub fn cover_thumbnail_state(&self, cover: &Cover) -> Option<&ThumbnailState> {
        self.thumbnails.entries.get(cover).map(|entry| &entry.state)
    }

    pub fn cover_thumbnail_path(&self, cover: &Cover) -> Option<&Path> {
        self.thumbnails.entries.get(cover)?.path.as_deref()
    }

    pub fn cover_thumbnail(&self, cover: &Cover) -> Option<CoverStatus> {
        let entry = self.thumbnails.entries.get(cover)?;
        Some(if let Some(image) = &entry.image {
            CoverStatus::Ready(image.clone())
        } else if matches!(entry.state, ThumbnailState::Failed(_)) {
            CoverStatus::Unavailable
        } else {
            CoverStatus::Loading
        })
    }

    pub(super) fn thumbnail_statuses(&self) -> HashMap<Cover, CoverStatus> {
        self.thumbnails
            .entries
            .keys()
            .filter_map(|cover| {
                self.cover_thumbnail(cover)
                    .map(|status| (cover.clone(), status))
            })
            .collect()
    }

    pub fn request_cover_thumbnail(&mut self, cover: Cover, cx: &mut Context<Self>) -> CoverStatus {
        self.thumbnails.clock += 1;
        let clock = self.thumbnails.clock;
        if let Some(entry) = self.thumbnails.entries.get_mut(&cover) {
            entry.used = clock;
            if entry.image.is_some()
                || entry.started
                || matches!(entry.state, ThumbnailState::Failed(_))
            {
                return self.cover_thumbnail(&cover).unwrap();
            }
            if matches!(entry.state, ThumbnailState::Loading) {
                return CoverStatus::Loading;
            }
        }
        self.thumbnails.entries.insert(
            cover.clone(),
            Entry {
                identity: clock,
                used: clock,
                state: ThumbnailState::Loading,
                path: None,
                image: None,
                started: false,
            },
        );
        self.thumbnails.queue.push_back(cover);
        self.start_cover_thumbnails(cx);
        CoverStatus::Loading
    }

    fn start_cover_thumbnails(&mut self, cx: &mut Context<Self>) {
        while self.thumbnails.active.len() < CONCURRENCY {
            let Some(cover) = self.thumbnails.queue.pop_front() else {
                break;
            };
            let Some(entry) = self.thumbnails.entries.get_mut(&cover) else {
                continue;
            };
            if entry.started {
                continue;
            }
            entry.started = true;
            let identity = entry.identity;
            self.thumbnails.active.insert(identity);
            let generation = self.thumbnails.generation.clone();
            let expected = *generation.lock().unwrap();
            let directory = self.thumbnails.directory.clone();
            let source = cover.clone();
            let task = cx.background_spawn(async move {
                load(&source, directory.as_deref(), &generation, expected)
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    this.thumbnails.active.remove(&identity);
                    if *this.thumbnails.generation.lock().unwrap() == expected
                        && let Some(entry) = this.thumbnails.entries.get_mut(&cover)
                        && entry.identity == identity
                    {
                        entry.started = false;
                        match result {
                            Ok((path, image)) => {
                                entry.path = path;
                                entry.image = Some(image);
                                entry.state = ThumbnailState::Ready;
                            }
                            Err(error) => entry.state = ThumbnailState::Failed(error),
                        }
                        while this
                            .thumbnails
                            .entries
                            .values()
                            .filter(|entry| entry.image.is_some())
                            .count()
                            > MEMORY_CAPACITY
                        {
                            if let Some(oldest) = this
                                .thumbnails
                                .entries
                                .iter()
                                .filter(|(_, entry)| entry.image.is_some())
                                .min_by_key(|(_, entry)| entry.used)
                                .map(|(cover, _)| cover.clone())
                            {
                                this.thumbnails.entries.get_mut(&oldest).unwrap().image = None;
                            }
                        }
                    }
                    this.start_cover_thumbnails(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

fn stamp(path: &Path) -> std::io::Result<String> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok(format!(
        "{}:{}:{}",
        metadata.len(),
        modified.as_secs(),
        modified.subsec_nanos()
    ))
}

fn digest(bytes: &[u8]) -> String {
    // Versioned local asset keys; manifests also validate the complete Cover.
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn decode(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("封面图片超过本地缩略图读取上限".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("无法识别封面图片：{error}"))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|error| format!("无法解码封面图片：{error}"))
}

fn rendered(decoded: image::DynamicImage) -> Arc<RenderImage> {
    let mut pixels = decoded.into_rgba8();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new([image::Frame::new(pixels)]))
}

fn cached(
    directory: &Path,
    key: &str,
    cover: &Cover,
    source_stamp: Option<&str>,
) -> Result<(PathBuf, image::DynamicImage), String> {
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.join(format!("{key}.json"))).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if record["cover"] != serde_json::to_value(cover).unwrap()
        || source_stamp.is_some_and(|stamp| record["stamp"].as_str() != Some(stamp))
    {
        return Err("封面来源已变化".into());
    }
    let filename = record["file"]
        .as_str()
        .filter(|filename| {
            Path::new(filename).components().count() == 1 && filename.ends_with(".png")
        })
        .ok_or("本地封面关联损坏")?;
    let path = directory.join(filename);
    let bytes = fs::read(&path).map_err(|error| format!("无法读取本地封面：{error}"))?;
    if filename != format!("{key}-{}.png", digest(&bytes)) {
        return Err("本地封面内容损坏，请在来源恢复后重试".into());
    }
    let decoded = decode(&bytes)?;
    Ok((path, decoded))
}

fn load(
    cover: &Cover,
    directory: Option<&Path>,
    generation: &Mutex<u64>,
    expected: u64,
) -> Result<(Option<PathBuf>, Arc<RenderImage>), String> {
    let source_stamp = stamp(cover.source.path()).ok();
    let key = digest(&serde_json::to_vec(cover).unwrap());
    if let Some(directory) = directory
        && let Ok((path, decoded)) = cached(directory, &key, cover, source_stamp.as_deref())
    {
        return Ok((Some(path), rendered(decoded)));
    }
    let bytes = voon_core::tags::read_cover(cover)
        .map_err(|error| format!("无法读取封面 {}：{error}", cover.source.path().display()))?;
    let decoded = decode(&bytes)
        .map_err(|error| format!("{error}\n{}", cover.source.path().display()))?
        .thumbnail(256, 256);
    if source_stamp.is_none() || stamp(cover.source.path()).ok() != source_stamp {
        return Err("封面来源在读取期间变化，请重试".into());
    }
    let Some(directory) = directory else {
        return Ok((None, rendered(decoded)));
    };
    fs::create_dir_all(directory)
        .map_err(|error| format!("无法创建本地封面目录 {}：{error}", directory.display()))?;
    let mut png = Cursor::new(Vec::new());
    decoded
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| format!("无法生成本地封面：{error}"))?;
    let bytes = png.into_inner();
    let filename = format!("{key}-{}.png", digest(&bytes));
    let path = directory.join(&filename);
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    );
    let image_temp = directory.join(format!("{suffix}.png.tmp"));
    let record_temp = directory.join(format!("{suffix}.json.tmp"));
    let record_path = directory.join(format!("{key}.json"));
    let record = serde_json::to_vec(
        &serde_json::json!({"cover": cover, "stamp": source_stamp, "file": filename}),
    )
    .unwrap();
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&image_temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        // Read the actual staged local asset before publishing its association.
        decode(&fs::read(&image_temp)?).map_err(std::io::Error::other)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&record_temp)?;
        file.write_all(&record)?;
        file.sync_all()?;
        let unchanged = stamp(cover.source.path()).ok() == source_stamp;
        let guard = generation.lock().unwrap();
        if *guard != expected || !unchanged {
            return Err(std::io::Error::other("封面任务已过期，请重试"));
        }
        fs::rename(&image_temp, &path)?;
        fs::rename(&record_temp, &record_path)?;
        Ok(())
    })();
    let _ = fs::remove_file(&image_temp);
    let _ = fs::remove_file(&record_temp);
    result.map_err(|error| format!("无法保存本地封面 {}：{error}", directory.display()))?;
    Ok((Some(path), rendered(decoded)))
}
