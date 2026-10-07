//! One page per dispatch with durable, bounded traversal and scope evidence.
use super::*;
use serde::Serialize;
use std::collections::HashSet;
use transport::Page;

const MAX_CURSOR: usize = 4096;
const MAX_PAGES: usize = 20;
const MAX_WORKSPACES: usize = 20;
const MAX_PENDING: usize = 10;

#[derive(Deserialize)]
struct Collection<T> {
    values: Vec<T>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceAccess {
    #[serde(rename = "type")]
    kind: String,
    workspace: Workspace,
}

#[derive(Deserialize)]
struct Workspace {
    #[serde(rename = "type")]
    kind: String,
    uuid: String,
    slug: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    account: String,
    epoch: String,
    kind: String,
    stage: Stage,
    pending: Vec<String>,
    outer: Option<String>,
    pages: usize,
    seen_pages: Vec<String>,
    seen_workspaces: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Stage {
    Workspaces { url: String },
    Repositories { workspace: String, url: String },
}

impl Stage {
    fn route(&self) -> Route {
        match self {
            Self::Workspaces { .. } => Route::Workspaces,
            Self::Repositories { workspace, .. } => Route::Repositories(workspace.clone()),
        }
    }

    fn url(&self) -> &str {
        match self {
            Self::Workspaces { url } | Self::Repositories { url, .. } => url,
        }
    }
}

impl Cursor {
    fn open(request: &FeedRequest, http: &BitbucketHttp) -> Result<Self, ProviderError> {
        if request.account.id.is_empty()
            || request.account.id.len() > 128
            || request.account.id.chars().any(char::is_control)
            || request
                .account
                .authorization_epoch
                .parse::<i64>()
                .ok()
                .filter(|n| *n > 0)
                .is_none_or(|n| n.to_string() != request.account.authorization_epoch)
        {
            return Err(invalid());
        }
        let Some(raw) = &request.cursor else {
            return Ok(Self {
                version: 1,
                account: request.account.id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                kind: "repositories".into(),
                stage: Stage::Workspaces {
                    url: http.endpoint(&Route::Workspaces)?.to_string(),
                },
                pending: vec![],
                outer: None,
                pages: 0,
                seen_pages: vec![],
                seen_workspaces: vec![],
            });
        };
        if raw.len() > MAX_CURSOR {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.account != request.account.id
            || cursor.epoch != request.account.authorization_epoch
            || cursor.kind != "repositories"
            || cursor.pages == 0
            || cursor.pages > MAX_PAGES
            || cursor.pages != cursor.seen_pages.len()
            || cursor.pending.len() > MAX_PENDING
            || cursor.seen_workspaces.len() > MAX_WORKSPACES
        {
            return Err(invalid());
        }
        let mut hashes = HashSet::new();
        for hash in &cursor.seen_pages {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
                || !hashes.insert(hash.clone())
            {
                return Err(invalid());
            }
        }
        let mut workspaces = HashSet::new();
        for workspace in &cursor.seen_workspaces {
            if canonical_uuid(workspace)? != *workspace || !workspaces.insert(workspace.clone()) {
                return Err(invalid());
            }
        }
        let mut pending = HashSet::new();
        for workspace in &cursor.pending {
            if !workspaces.contains(workspace) || !pending.insert(workspace.clone()) {
                return Err(invalid());
            }
        }
        match &cursor.stage {
            Stage::Workspaces { url } => {
                if !cursor.pending.is_empty() || cursor.outer.is_some() {
                    return Err(invalid());
                }
                http.continuation(url, &Route::Workspaces)?;
            }
            Stage::Repositories { workspace, url } => {
                if !workspaces.contains(workspace) || pending.contains(workspace) {
                    return Err(invalid());
                }
                // Initial repository routes and opaque continuations share
                // the exact trusted UUID endpoint and immutable member filter.
                http.fingerprint(url, &Route::Repositories(workspace.clone()))?;
            }
        }
        if let Some(outer) = &cursor.outer {
            http.continuation(outer, &Route::Workspaces)?;
        }
        let hash = http.fingerprint(cursor.stage.url(), &cursor.stage.route())?;
        if hashes.contains(&hash) || cursor.pages >= MAX_PAGES {
            // A capped persisted traversal cannot reset itself on a manual
            // refresh or restart. Retain its previous Partial/cache evidence.
            return Err(invalid());
        }
        Ok(cursor)
    }

    fn next(&mut self, http: &BitbucketHttp) -> Result<Option<String>, ProviderError> {
        if let Some(workspace) = self.pending.first().cloned() {
            self.pending.remove(0);
            self.stage = Stage::Repositories {
                url: http
                    .endpoint(&Route::Repositories(workspace.clone()))?
                    .to_string(),
                workspace,
            };
        } else if let Some(url) = self.outer.take() {
            self.stage = Stage::Workspaces { url };
        } else {
            return Ok(None);
        }
        self.encoded(http).map(Some)
    }

    fn encoded(&self, http: &BitbucketHttp) -> Result<String, ProviderError> {
        let hash = http.fingerprint(self.stage.url(), &self.stage.route())?;
        if self.seen_pages.contains(&hash) {
            return Err(invalid());
        }
        let encoded = serde_json::to_string(self).map_err(|_| invalid())?;
        if encoded.len() > MAX_CURSOR {
            return Err(invalid());
        }
        Ok(encoded)
    }
}

impl BitbucketCloudProvider {
    pub(super) async fn discover(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        if request.account.provider != ProviderKind::BitbucketCloud
            || request.account.host != "bitbucket.org"
            || request.kind != FeedKind::Repositories
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.account.state != AccountState::Active {
            return Err(ProviderError::new(ProviderErrorKind::Authentication));
        }
        if request.repository.is_some() {
            return Err(invalid());
        }
        canonical_uuid(&request.account.actor_id)?;
        let mut cursor = Cursor::open(&request, &self.http)?;
        let route = cursor.stage.route();
        let url = reqwest::Url::parse(cursor.stage.url()).map_err(|_| invalid())?;
        let page = self.http.get(url, &route, token).await?;
        let result = (|| {
            cursor
                .seen_pages
                .push(self.http.fingerprint(cursor.stage.url(), &route)?);
            cursor.pages += 1;
            let rows;
            let next_cursor = match &route {
                Route::Workspaces => {
                    let ids = workspaces(&page, &self.http, &route)?;
                    if ids.iter().any(|id| cursor.seen_workspaces.contains(id))
                        || cursor.seen_workspaces.len() + ids.len() > MAX_WORKSPACES
                    {
                        return Err(invalid());
                    }
                    cursor.seen_workspaces.extend(ids.clone());
                    cursor.pending = ids;
                    cursor.outer = collection_next::<WorkspaceAccess>(&page, &self.http, &route)?;
                    rows = vec![];
                    cursor.next(&self.http)?
                }
                Route::Repositories(workspace) => {
                    rows = repositories(&page, &request.account.id, workspace, &self.http, &route)?;
                    if let Some(url) = collection_next::<Repository>(&page, &self.http, &route)? {
                        cursor.stage = Stage::Repositories {
                            workspace: workspace.clone(),
                            url,
                        };
                        Some(cursor.encoded(&self.http)?)
                    } else {
                        cursor.next(&self.http)?
                    }
                }
                Route::User
                | Route::PullRequests(_)
                | Route::PullRequest(..)
                | Route::Tasks(..)
                | Route::Commits(..)
                | Route::PullFiles { .. } => {
                    return Err(invalid());
                }
            };
            Ok(FetchPage {
                repositories: rows,
                items: vec![],
                endpoint_aliases: vec![],
                notification_subjects: vec![],
                next_cursor,
                etag: None,
                last_modified: None,
                not_modified: false,
                poll_interval_seconds: None,
                cooldown_seconds: page.cooldown,
            })
        })();
        result.map_err(|error| quota(error, page.cooldown))
    }
}

fn collection_next<T: for<'de> Deserialize<'de>>(
    page: &Page,
    http: &BitbucketHttp,
    route: &Route,
) -> Result<Option<String>, ProviderError> {
    let collection: Collection<T> = serde_json::from_slice(&page.body).map_err(|_| invalid())?;
    collection
        .next
        .map(|raw| http.continuation(&raw, route).map(|url| url.to_string()))
        .transpose()
}

pub(super) fn workspaces(
    page: &Page,
    http: &BitbucketHttp,
    route: &Route,
) -> Result<Vec<String>, ProviderError> {
    let collection: Collection<WorkspaceAccess> =
        serde_json::from_slice(&page.body).map_err(|_| invalid())?;
    if collection.values.len() > MAX_PENDING {
        return Err(quota(invalid(), page.cooldown));
    }
    if let Some(next) = collection.next {
        http.continuation(&next, route)?;
    }
    let mut seen = HashSet::new();
    collection
        .values
        .into_iter()
        .map(|access| {
            let workspace = access.workspace;
            let id = canonical_uuid(&workspace.uuid)?;
            if access.kind != "workspace_access"
                || !matches!(workspace.kind.as_str(), "workspace" | "workspace_base")
                || !segment(&workspace.slug)
                || !seen.insert(id.clone())
            {
                return Err(invalid());
            }
            Ok(id)
        })
        .collect()
}

pub(super) fn repositories(
    page: &Page,
    account: &str,
    workspace: &str,
    http: &BitbucketHttp,
    route: &Route,
) -> Result<Vec<RemoteRepository>, ProviderError> {
    let collection: Collection<Repository> =
        serde_json::from_slice(&page.body).map_err(|_| invalid())?;
    if collection.values.len() > 50 {
        return Err(invalid());
    }
    if let Some(next) = collection.next {
        http.continuation(&next, route)?;
    }
    let mut seen = HashSet::new();
    collection
        .values
        .into_iter()
        .map(|repo| {
            let remote = repo.remote(account, workspace)?;
            if !seen.insert(remote.provider_id.clone()) {
                return Err(invalid());
            }
            Ok(remote)
        })
        .collect()
}

#[derive(Deserialize)]
struct Repository {
    #[serde(rename = "type")]
    kind: String,
    uuid: String,
    scm: String,
    name: String,
    full_name: String,
    workspace: Workspace,
    links: Links,
    description: Option<String>,
    mainbranch: Option<BitbucketDefaultBranch>,
}

#[derive(Deserialize)]
struct Links {
    html: Link,
    clone: Option<Vec<CloneLink>>,
}
#[derive(Deserialize)]
struct Link {
    href: String,
}
#[derive(Deserialize)]
struct CloneLink {
    name: String,
    href: String,
}
#[derive(Deserialize)]
struct BitbucketDefaultBranch {
    name: String,
}

impl Repository {
    fn remote(self, account: &str, workspace: &str) -> Result<RemoteRepository, ProviderError> {
        let id = canonical_uuid(&self.uuid)?;
        let parts: Vec<_> = self.full_name.split('/').collect();
        if self.kind != "repository"
            || self.scm != "git"
            || canonical_uuid(&self.workspace.uuid)? != workspace
            || !matches!(self.workspace.kind.as_str(), "workspace" | "workspace_base")
            || parts.len() != 2
            || !parts.iter().all(|part| segment(part))
            || parts[0] != self.workspace.slug
        {
            return Err(invalid());
        }
        let web_url = format!("https://bitbucket.org/{}", self.full_name);
        if !web_link(&self.links.html.href, &web_url) {
            return Err(invalid());
        }
        if let Some(clones) = self.links.clone
            && (clones.len() > 4
                || clones
                    .into_iter()
                    .any(|link| !clone_link(link, &self.full_name)))
        {
            return Err(invalid());
        }
        let description = self
            .description
            .map(|value| {
                if value.len() > 16 * 1024 || value.contains('\0') {
                    Err(invalid())
                } else {
                    Ok(value)
                }
            })
            .transpose()?;
        Ok(RemoteRepository {
            id: format!("bitbucket_cloud:repository:{id}"),
            account_id: account.into(),
            provider_id: id,
            full_name: self.full_name,
            name: text(self.name, 1024, false)?,
            web_url,
            description,
            default_branch: self
                .mainbranch
                .map(|branch| text(branch.name, 1024, false))
                .transpose()?,
            selected: false,
        })
    }
}

pub(super) fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn web_link(raw: &str, expected: &str) -> bool {
    (raw == expected || raw == format!("{expected}/"))
        && reqwest::Url::parse(raw).is_ok_and(|url| {
            url.username().is_empty() && url.password().is_none() && url.port().is_none()
        })
}

fn clone_link(link: CloneLink, full_name: &str) -> bool {
    if link.href.len() > 2048
        || link
            .href
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
    {
        return false;
    }
    match link.name.as_str() {
        "ssh" => {
            link.href == format!("git@bitbucket.org:{full_name}.git")
                || link.href == format!("ssh://git@bitbucket.org/{full_name}.git")
        }
        "https" => reqwest::Url::parse(&link.href).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("bitbucket.org")
                && url.port().is_none()
                && url.password().is_none()
                && url.username().len() <= 255
                && !url.username().contains('%')
                && url.path() == format!("/{full_name}.git")
                && url.query().is_none()
                && url.fragment().is_none()
                && !link.href.contains('\\')
        }),
        _ => false,
    }
}
