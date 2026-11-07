use std::borrow::Cow;

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use rust_embed::RustEmbed;
use serde_json::json;

use crate::state::AppState;

pub mod base_path;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const SETTINGS_HTML: &str = r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>面板设置</title>
<style>
body{margin:0;background:#0b1220;color:#d6e2f0;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","Microsoft YaHei",sans-serif}
.wrap{max-width:760px;margin:0 auto;padding:24px 16px}
.bar{display:flex;gap:12px;align-items:center;margin-bottom:20px;flex-wrap:wrap}
.bar strong{font-size:18px}.bar .spacer{flex:1}
a,button{background:#1f2a3d;color:#d6e2f0;border:1px solid #2d3b52;border-radius:6px;padding:7px 12px;font-size:13px;cursor:pointer;text-decoration:none}
button.primary{background:#1677ff;border-color:#1677ff}button:hover,a:hover{filter:brightness(1.15)}
.card{background:#111a2b;border:1px solid #1f2a3d;border-radius:8px;padding:16px;margin-bottom:16px}
.card h3{margin:0 0 12px;font-size:15px}
label{display:block;color:#8fa3bf;font-size:12px;margin:10px 0 4px}
input[type=text],input[type=password],input[type=file]{width:100%;box-sizing:border-box;background:#0b1220;color:#d6e2f0;border:1px solid #2d3b52;border-radius:6px;padding:8px 10px}
.row{display:flex;gap:10px;flex-wrap:wrap}.row>*{flex:1;min-width:180px}
#msg{min-height:20px;font-size:13px;margin:10px 0}#msg.err{color:#ff7875}#msg.ok{color:#95de64}
.note{color:#8fa3bf;font-size:12px;line-height:1.6}
</style></head><body><div class="wrap">
<div class="bar"><strong>面板设置</strong><span class="spacer"></span><a href="__BASE__/panel/inbounds">返回面板</a><button id="logout">退出登录</button></div>
<div id="msg"></div>
<div class="card"><h3>管理员账号</h3>
<label>用户名</label><input type="text" id="username" autocomplete="username">
<div class="row"><div><label>当前密码</label><input type="password" id="current" autocomplete="current-password"></div>
<div><label>新密码（至少 8 位，留空不改）</label><input type="password" id="next" autocomplete="new-password"></div></div>
<p><button class="primary" id="save">保存账号</button></p>
<p class="note">修改成功后会用新账号重新登录；旧会话全部失效。</p></div>
<div class="card"><h3>数据库备份</h3>
<p class="note">下载 SQLite 数据库文件；恢复会覆盖当前面板数据并重启 Xray。</p>
<p><a href="__BASE__/api/server/db">下载 .db 备份</a></p>
<label>选择 .db 文件恢复</label><input type="file" id="dbfile" accept=".db,application/octet-stream">
<p><button id="restore">上传并恢复</button></p></div>
<script>
var $=function(id){return document.getElementById(id)};
var B='__BASE__';
var csrf='';
function show(t,e){var m=$('msg');m.textContent=t;m.className=e?'err':'ok'}
async function boot(){var r=await fetch(B+'/api/session',{credentials:'same-origin'});var j=await r.json().catch(function(){return null});if(!j||!j.authenticated){location.href=B+'/';return}csrf=j.csrf||'';$('username').value=j.username||''}
$('save').onclick=async function(){
  if(!$('current').value){show('请输入当前密码',true);return}
  var body={username:$('username').value,currentPassword:$('current').value};
  if($('next').value)body.newPassword=$('next').value;
  var r=await fetch(B+'/api/credentials',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json','X-CSRF-Token':csrf},body:JSON.stringify(body)});
  var j=await r.json().catch(function(){return null});
  if(r.ok){show('保存成功，请使用新账号重新登录',false);$('current').value='';$('next').value='';if(j&&j.csrf)csrf=j.csrf;setTimeout(function(){location.href='/';},1200)}else{show((j&&j.error)||'保存失败',true)}
};
$('restore').onclick=async function(){
  var f=$('dbfile').files[0];if(!f){show('请选择 .db 文件',true);return}
  if(!confirm('恢复会覆盖当前数据库，确定继续？'))return;
  var fd=new FormData();fd.append('file',f);
  var r=await fetch(B+'/api/server/db/restore',{method:'POST',credentials:'same-origin',headers:{'X-CSRF-Token':csrf},body:fd});
  var j=await r.json().catch(function(){return null});
  if(r.ok){show('恢复成功，Xray 已重启',false)}else{show((j&&j.error)||'恢复失败',true)}
};
$('logout').onclick=async function(){await fetch(B+'/api/logout',{method:'POST',credentials:'same-origin',headers:{'X-CSRF-Token':csrf}});location.href=B+'/'};
boot();
</script></div></body></html>"#;

pub async fn static_handler(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let base = state.base_path();

    if path.starts_with("api/") {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response();
    }

    if path == "favicon.ico" {
        return StatusCode::NO_CONTENT.into_response();
    }
    if path.is_empty() {
        return serve_entry("login.html", &base);
    }
    if path == "login" || path == "login.html" {
        return serve_entry("login.html", &base);
    }
    if path == "panel" || path == "panel/" || path.starts_with("panel/") {
        return serve_entry("index.html", &base);
    }

    if let Some(content) = Assets::get(path) {
        // Release builds embed the bytes statically; copying a 1.3 MB vendor
        // bundle into a Vec per request was the largest single RSS step seen
        // in the 0.2.40 memory probe. Keep the borrowed slice and only move
        // the Vec when rust-embed had to read from disk (debug builds).
        return serve(path, content.data);
    }

    let last = path.rsplit('/').next().unwrap_or(path);
    if !last.contains('.') {
        return serve_entry("index.html", &base);
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

pub async fn login_page(State(state): State<AppState>) -> Response {
    serve_entry("login.html", &state.base_path())
}

pub async fn settings_page(State(state): State<AppState>) -> Response {
    // The script tag and links live in the same static template, so one
    // placeholder covers both; base_prefix is "" for the default path.
    let base_prefix = state.base_path().trim_end_matches('/').to_string();
    let html = SETTINGS_HTML.replace("__BASE__", &base_prefix);
    let mut response = Response::new(Body::from(html));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

fn serve_entry(name: &str, base: &str) -> Response {
    let Some(content) = Assets::get(name) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let html = String::from_utf8_lossy(&content.data);
    // Vite emits root-absolute asset URLs; they must live under the panel
    // prefix too, otherwise the SPA cannot boot behind a custom path.
    let html = if base == "/" {
        html.into_owned()
    } else {
        html.replace("=\"/assets/", &format!("=\"{base}assets/"))
            .replace("='/assets/", &format!("='{base}assets/"))
    };
    let injected = html.replacen(
        "</head>",
        &format!(
            "<script>window.X_UI_BASE_PATH=\"{base}\";window.X_UI_CUR_VER=\"{VERSION}\";</script></head>"
        ),
        1,
    );
    let mut response = Response::new(Body::from(injected));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

fn serve(path: &str, data: Cow<'static, [u8]>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let bytes: Bytes = match data {
        Cow::Borrowed(slice) => Bytes::from_static(slice),
        Cow::Owned(vec) => Bytes::from(vec),
    };
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    let cache = if path.ends_with(".html") {
        HeaderValue::from_static("no-cache")
    } else {
        HeaderValue::from_static("public, max-age=31536000, immutable")
    };
    response.headers_mut().insert(header::CACHE_CONTROL, cache);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn borrowed_assets_keep_their_bytes_without_a_copy() {
        let response = serve("x.js", Cow::Borrowed(&b"hello"[..]));
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("body");
        assert_eq!(&body[..], b"hello");
    }

    #[tokio::test]
    async fn owned_assets_are_moved_into_the_response() {
        let response = serve("x.css", Cow::Owned(b"owned".to_vec()));
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("body");
        assert_eq!(&body[..], b"owned");
    }
}
