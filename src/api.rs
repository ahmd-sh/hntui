// HN Firebase API: typed errors, retry/timeout policy, and a shared item
// cache with request de-duplication — the same contract as src/api/hn.ts.
//
// Effect gave the TS version interruption for free; here it falls out of
// Rust futures being lazy: aborting a task drops its future, which drops
// the in-flight reqwest request.

use futures::future::{BoxFuture, FutureExt, Shared};
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::format::HnItemRef;

const BASE: &str = "https://hacker-news.firebaseio.com/v0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeedCategory {
    Top,
    New,
    Best,
    Ask,
    Show,
    Job,
}

impl FeedCategory {
    pub const ALL: [FeedCategory; 6] =
        [FeedCategory::Top, FeedCategory::New, FeedCategory::Best, FeedCategory::Ask, FeedCategory::Show, FeedCategory::Job];

    pub fn label(self) -> &'static str {
        match self {
            FeedCategory::Top => "Top",
            FeedCategory::New => "New",
            FeedCategory::Best => "Best",
            FeedCategory::Ask => "Ask",
            FeedCategory::Show => "Show",
            FeedCategory::Job => "Jobs",
        }
    }

    fn path(self) -> &'static str {
        match self {
            FeedCategory::Top => "topstories.json",
            FeedCategory::New => "newstories.json",
            FeedCategory::Best => "beststories.json",
            FeedCategory::Ask => "askstories.json",
            FeedCategory::Show => "showstories.json",
            FeedCategory::Job => "jobstories.json",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    Story,
    Comment,
    Job,
    Poll,
    Pollopt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Item {
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: ItemType,
    pub by: Option<String>,
    pub time: Option<i64>,
    pub text: Option<String>,
    #[serde(default)]
    pub dead: bool,
    #[serde(default)]
    pub deleted: bool,
    pub parent: Option<u64>,
    pub kids: Option<Vec<u64>>,
    pub url: Option<String>,
    pub score: Option<i64>,
    pub title: Option<String>,
    pub descendants: Option<i64>,
}

impl Item {
    fn is_comment(&self) -> bool {
        matches!(self.kind, ItemType::Comment | ItemType::Pollopt)
    }
}

/// Every way a call can fail. `match` on this is exhaustive at compile time,
/// which is what Effect's Match.exhaustive bought the TS version.
#[derive(Clone, Debug)]
pub enum HnError {
    Request,
    Status(u16),
    Timeout,
    Decode,
}

#[derive(Clone, Debug)]
pub enum ResolveError {
    Hn(HnError),
    // not an API failure: the link points at nothing viewable
    Gone,
}

impl From<HnError> for ResolveError {
    fn from(e: HnError) -> Self {
        ResolveError::Hn(e)
    }
}

pub fn error_message(e: &HnError) -> String {
    match e {
        HnError::Request => "Couldn't reach Hacker News — check your connection.".into(),
        HnError::Timeout => "Hacker News took too long to respond.".into(),
        HnError::Status(s) => format!("Hacker News returned HTTP {s}."),
        HnError::Decode => "Hacker News sent a response the app couldn't understand.".into(),
    }
}

pub struct Resolved {
    pub story: Arc<Item>,
    pub focus_id: Option<u64>,
}

type ItemResult = Result<Option<Arc<Item>>, HnError>;

pub struct Hn {
    http: reqwest::Client,
    cache: Mutex<HashMap<u64, Shared<BoxFuture<'static, ItemResult>>>>,
    // HN_DEMO=1: the story to splice the demo comment into (see `demo`)
    demo: Option<Mutex<Option<u64>>>,
}

// Demo mode, for trying link handling without hunting for the right
// comment: live data, plus one synthetic comment at the top of the first
// story of the first feed loaded. Its four links cover each case: an HN
// story (opens in the app), an HN comment (opens its story, cursor on the
// comment), a dead HN link (the not-found screen) and an external link.
//
//   HN_DEMO=1 hntui
const DEMO_ID: u64 = 999_999_001;

fn demo_comment(parent: Option<u64>) -> Item {
    let text = concat!(
        "[SIMULATED COMMENT] Press ⏎ on me to list my links: ",
        r#"<a href="https://news.ycombinator.com/item?id=1">the first HN post ever</a>, "#,
        r#"<a href="https://news.ycombinator.com/item?id=15">a comment under it</a>, "#,
        r#"<a href="https://news.ycombinator.com/item?id=999999999999">a broken HN link</a>, "#,
        r#"and <a href="https://example.com/">an external link</a>."#,
    );
    serde_json::from_value(serde_json::json!({
        "id": DEMO_ID, "type": "comment", "by": "link-demo", "time": crate::format::now_secs(),
        "parent": parent, "text": text,
    }))
    .expect("demo comment")
}

impl Hn {
    pub fn new() -> Arc<Hn> {
        let http = reqwest::Client::builder().user_agent("hntui").build().expect("http client");
        let demo = (std::env::var("HN_DEMO").as_deref() == Ok("1")).then(|| Mutex::new(None));
        Arc::new(Hn { http, cache: Mutex::new(HashMap::new()), demo })
    }

    fn demo_story(&self) -> Option<u64> {
        *self.demo.as_ref()?.lock().unwrap()
    }

    // one attempt, 5s budget
    async fn attempt(&self, path: &str) -> Result<Vec<u8>, HnError> {
        let fut = async {
            let res = self.http.get(format!("{BASE}/{path}")).send().await.map_err(|_| HnError::Request)?;
            if !res.status().is_success() {
                return Err(HnError::Status(res.status().as_u16()));
            }
            res.bytes().await.map(|b| b.to_vec()).map_err(|_| HnError::Request)
        };
        tokio::time::timeout(Duration::from_secs(5), fut).await.unwrap_or(Err(HnError::Timeout))
    }

    // transient failures retry with exponential backoff (250ms, 500ms);
    // client errors (4xx) don't
    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, HnError> {
        let mut delay = Duration::from_millis(250);
        let mut tries = 0;
        let bytes = loop {
            match self.attempt(path).await {
                Ok(b) => break b,
                Err(HnError::Status(s)) if s < 500 => return Err(HnError::Status(s)),
                Err(e) if tries >= 2 => return Err(e),
                Err(_) => {
                    tries += 1;
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        };
        serde_json::from_slice(&bytes).map_err(|_| HnError::Decode)
    }

    pub async fn fetch_ids(&self, cat: FeedCategory) -> Result<Vec<u64>, HnError> {
        // HN answers null (with a 200) for missing data
        let ids = self.get_json::<Option<Vec<u64>>>(cat.path()).await?.unwrap_or_default();
        if let (Some(hook), Some(&first)) = (&self.demo, ids.first()) {
            hook.lock().unwrap().get_or_insert(first);
        }
        Ok(ids)
    }

    /// Concurrent lookups of one id share a request; successes are memoized,
    /// failures evicted so the next attempt refetches.
    pub async fn fetch_item(self: &Arc<Self>, id: u64) -> ItemResult {
        let fut = {
            let mut cache = self.cache.lock().unwrap();
            cache
                .entry(id)
                .or_insert_with(|| {
                    let me = self.clone();
                    async move {
                        if me.demo.is_some() && id == DEMO_ID {
                            return Ok(Some(Arc::new(demo_comment(me.demo_story()))));
                        }
                        let item = me.get_json::<Option<Item>>(&format!("item/{id}.json")).await?;
                        Ok(item.map(|mut it| {
                            it.title = it.title.as_deref().map(crate::format::clean_text);
                            it.text = it.text.as_deref().map(crate::format::clean_text);
                            if me.demo_story() == Some(id) {
                                it.kids = Some([DEMO_ID].into_iter().chain(it.kids.take().unwrap_or_default()).collect());
                                it.descendants = Some(it.descendants.unwrap_or(0) + 1);
                            }
                            Arc::new(it)
                        }))
                    }
                    .boxed()
                    .shared()
                })
                .clone()
        };
        let res = fut.await;
        if res.is_err() {
            self.cache.lock().unwrap().remove(&id);
        }
        res
    }

    /// Many items, ≤ `concurrency` in flight, failures and nulls dropped,
    /// input order kept.
    pub async fn fetch_items(self: &Arc<Self>, ids: Vec<u64>, concurrency: usize) -> Vec<Arc<Item>> {
        stream::iter(ids)
            .map(|id| self.fetch_item(id))
            .buffered(concurrency)
            .filter_map(|r| async move { r.ok().flatten() })
            .collect()
            .await
    }

    /// An /item?id=N link may point at a comment: walk `parent` up to the
    /// root story.
    pub async fn resolve_story(self: &Arc<Self>, r: HnItemRef) -> Result<Resolved, ResolveError> {
        let first = self.fetch_item(r.id).await?.ok_or(ResolveError::Gone)?;
        let from_comment = first.is_comment().then_some(first.id);
        let mut cur = first;
        let mut hops = 0;
        while cur.is_comment() {
            hops += 1;
            let parent = match cur.parent {
                Some(p) if hops <= 64 => p,
                _ => return Err(ResolveError::Gone),
            };
            cur = self.fetch_item(parent).await?.ok_or(ResolveError::Gone)?;
        }
        if cur.deleted || cur.dead {
            return Err(ResolveError::Gone);
        }
        Ok(Resolved { story: cur, focus_id: r.anchor_id.or(from_comment) })
    }
}

/// releases/latest redirects to …/tag/vX.Y.Z: the version is in a header.
pub async fn fetch_latest_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .user_agent("hntui")
        .build()
        .ok()?;
    let res = client.get("https://github.com/ahmd-sh/hntui/releases/latest").send().await.ok()?;
    let loc = res.headers().get("location")?.to_str().ok()?;
    let v = loc.rsplit_once("/tag/v")?.1;
    let ok = v.split('.').count() == 3 && v.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    ok.then(|| v.to_string())
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let p = |s: &str| s.split('.').map(|x| x.parse::<u64>().unwrap_or(0)).chain(std::iter::repeat(0)).take(3).collect::<Vec<_>>();
    p(a).cmp(&p(b))
}
