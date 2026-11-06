use axum::{
    extract::{Query, Request},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    api::compat::{self, fail, ok},
    geodata::Store,
};

const MAX_TOKENS: usize = 512;

#[derive(Deserialize)]
pub struct CategoriesQuery {
    file: Option<String>,
    q: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct EntriesQuery {
    file: Option<String>,
    code: Option<String>,
    q: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

pub async fn files() -> Json<Value> {
    respond(run(|| Store::asset().files()).await)
}

pub async fn categories(Query(query): Query<CategoriesQuery>) -> Json<Value> {
    let Some(file) = file_param(query.file) else {
        return fail("缺少 Geodata 文件名");
    };
    let query_text = query.q.unwrap_or_default();
    let offset = query.offset.unwrap_or(0).max(0) as usize;
    let limit = query.limit.unwrap_or(0).max(0) as usize;
    respond(run(move || Store::asset().categories(&file, &query_text, offset, limit)).await)
}

pub async fn entries(Query(query): Query<EntriesQuery>) -> Json<Value> {
    let Some(file) = file_param(query.file) else {
        return fail("缺少 Geodata 文件名");
    };
    let Some(code) = query
        .code
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return fail("缺少 Geodata 分类代码");
    };
    let query_text = query.q.unwrap_or_default();
    let offset = query.offset.unwrap_or(0).max(0) as usize;
    let limit = query.limit.unwrap_or(0).max(0) as usize;
    respond(run(move || Store::asset().entries(&file, &code, &query_text, offset, limit)).await)
}

pub async fn validate(req: Request) -> Json<Value> {
    let params = match compat::read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let is_ip = params
        .get("kind")
        .is_some_and(|kind| kind.eq_ignore_ascii_case("ip"));
    let tokens: Vec<String> = params
        .get("tokens")
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .take(MAX_TOKENS)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let issues = run(move || Ok::<_, String>(Store::asset().validate(is_ip, &tokens))).await;
    respond(issues)
}

fn file_param(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn run<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|err| format!("Geodata 任务执行失败：{err}"))?
}

fn respond<T: serde::Serialize>(result: Result<T, String>) -> Json<Value> {
    match result {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => ok(value),
            Err(err) => fail(format!("序列化 Geodata 响应失败：{err}")),
        },
        Err(err) => fail(err),
    }
}
