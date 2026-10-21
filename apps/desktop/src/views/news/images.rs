//! The articles' pictures (= the image half of `news.svelte.ts`): the hub downloads each
//! once into its cache folder and answers with the file; the window turns the file into a
//! texture through [`crate::parts::image`] (read and decoded on the runtime, uploaded
//! from there, kept by path). The ones already on disk are resolved in one call when the
//! list arrives; the rest are fetched six at a time, each URL once.

use std::sync::Arc;

use dz_hub::Hub;
use egui::mutex::Mutex;
use egui::{Context, TextureHandle};
use rustc_hash::FxHashMap;
use tokio::sync::Semaphore;

/// Downloads at once (`MAX_CONCURRENT`).
const MAX_CONCURRENT: usize = 6;
/// URLs remembered before the table starts over (a long session of refreshes).
const KEEP: usize = 512;

#[derive(Clone)]
enum Img {
    Pending,
    /// The cached file.
    Ready(Arc<str>),
    /// Not to be asked again (hidden, as the Svelte view hides a picture that failed).
    Failed,
}

/// Remote URL → the file it was cached into. Cheap to clone: shared inside.
#[derive(Clone)]
pub struct Images {
    map: Arc<Mutex<FxHashMap<String, Img>>>,
    gate: Arc<Semaphore>,
}

impl Default for Images {
    fn default() -> Self {
        Self {
            map: Arc::default(),
            gate: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        }
    }
}

impl Images {
    /// The cached file for `url`, asking for it when it was never asked for.
    pub fn want(&self, hub: &Hub, url: &str) -> Option<Arc<str>> {
        if url.is_empty() {
            return None;
        }
        let mut map = self.map.lock();
        match map.get(url) {
            Some(Img::Ready(p)) => return Some(Arc::clone(p)),
            Some(Img::Pending | Img::Failed) => return None,
            None => {}
        }
        if map.len() >= KEEP {
            map.retain(|_, e| !matches!(e, Img::Failed));
        }
        map.insert(url.to_owned(), Img::Pending);
        drop(map);
        let (me, hub, url) = (self.clone(), hub.clone(), url.to_owned());
        crate::rt::fire(async move {
            // The gate is never closed; without a permit the download simply goes ahead.
            let _permit = me.gate.acquire().await.ok();
            let entry = match hub.fetch_image(url.clone()).await {
                Ok(p) => Img::Ready(p.to_string_lossy().into()),
                Err(_) => Img::Failed,
            };
            me.map.lock().insert(url, entry);
        });
        None
    }

    /// The picture at `url` as a texture, once it is downloaded and decoded.
    pub fn texture(&self, ctx: &Context, hub: &Hub, url: &str) -> Option<TextureHandle> {
        let path = self.want(hub, url)?;
        crate::parts::image::texture(ctx, &path)
    }

    /// Whether `url` is known not to come.
    pub fn failed(&self, url: &str) -> bool {
        matches!(self.map.lock().get(url), Some(Img::Failed))
    }

    /// Everything already cached, in one call; then fetch the missing ones. Runs on the
    /// runtime (called from the load).
    pub async fn warm(&self, hub: &Hub, urls: Vec<String>) {
        if urls.is_empty() {
            return;
        }
        if let Ok(cached) = hub.resolve_cached_images(urls.clone()).await {
            let mut map = self.map.lock();
            for (url, path) in cached {
                map.insert(url, Img::Ready(path.to_string_lossy().into()));
            }
        }
        for u in &urls {
            let _ = self.want(hub, u);
        }
    }
}
