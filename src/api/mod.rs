pub mod auth;
pub mod backup;
pub mod clients;
pub mod compat;
pub mod dashboard;
pub mod inbounds;
pub mod logs;
pub mod outbound_subs;
pub mod outbound_test;
pub mod public_ip;
pub mod settings;
pub mod xray;
pub mod xray_settings;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post, put},
    Json, Router,
};
use serde_json::json;

use crate::{auth::require_auth, state::AppState, web};

pub fn router(state: AppState) -> Router {
    let public = Router::new()
        .route("/api/health", get(health))
        .route("/api/login", post(auth::login))
        .route("/api/session", get(auth::session_info))
        .route("/csrf-token", get(compat::csrf_token))
        .route("/panel/csrf-token", get(compat::csrf_token))
        .route("/login", get(web::login_page).post(compat::login))
        .route("/settings", get(web::settings_page))
        .route("/logout", post(compat::logout));

    let protected = Router::new()
        .route("/api/logout", post(auth::logout))
        .route("/api/credentials", post(auth::change_credentials))
        .route("/api/dashboard", get(dashboard::dashboard))
        .route("/api/inbounds", get(inbounds::list).post(inbounds::create))
        .route(
            "/api/inbounds/{id}",
            get(inbounds::get)
                .put(inbounds::update)
                .delete(inbounds::delete),
        )
        .route("/api/inbounds/{id}/toggle", post(inbounds::toggle))
        .route("/api/inbounds/{id}/links", get(inbounds::links))
        .route("/api/clients", get(clients::list).post(clients::create))
        .route("/api/clients/online", get(clients::online))
        .route(
            "/api/clients/{id}",
            get(clients::get)
                .put(clients::update)
                .delete(clients::delete),
        )
        .route("/api/clients/{id}/toggle", post(clients::toggle))
        .route("/api/clients/{id}/reset", post(clients::reset))
        .route("/api/settings", get(settings::get).put(settings::update))
        .route("/api/xray/status", get(xray::status))
        .route("/api/xray/config", get(xray::config))
        .route("/api/xray/test", post(xray::test))
        .route("/api/xray/restart", post(xray::restart))
        .route("/api/xray/logs", get(xray::logs))
        .route("/api/xray/new-x25519", post(xray::new_x25519))
        .route(
            "/api/xray/template",
            get(xray_settings::template).put(xray_settings::put_template),
        )
        .route("/api/xray/outbounds", put(xray_settings::put_outbounds))
        .route(
            "/api/xray/outbounds/reset",
            post(xray_settings::reset_outbound_all),
        )
        .route(
            "/api/xray/outbounds/{tag}/reset",
            post(xray_settings::reset_outbound),
        )
        .route("/api/xray/routing", put(xray_settings::put_routing))
        .route("/api/xray/dns", put(xray_settings::put_dns))
        .route("/api/xray/log", put(xray_settings::put_log))
        .route("/panel/api/status", get(compat::status))
        .route("/panel/api/setting/all", post(compat::setting_all))
        .route("/panel/api/setting/update", post(compat::setting_update))
        .route(
            "/panel/api/setting/defaultSettings",
            post(compat::setting_defaults),
        )
        .route(
            "/panel/api/setting/factoryDefaults",
            post(compat::factory_defaults),
        )
        .route(
            "/panel/api/setting/getDefaultJsonConfig",
            get(compat::default_json_config).post(compat::default_json_config),
        )
        .route("/api/server/db", get(backup::download))
        .route("/api/server/db/restore", post(backup::restore))
        .route("/panel/api/server/getDb", get(backup::download))
        .route("/panel/api/server/importDB", post(backup::compat_import))
        .route("/panel/api/server/status", get(compat::server_status))
        .route(
            "/panel/api/server/getConfigJson",
            get(compat::server_config_json),
        )
        .route(
            "/panel/api/server/getXrayVersion",
            get(compat::server_xray_versions),
        )
        .route(
            "/panel/api/server/installXray/{version}",
            post(compat::server_install_xray),
        )
        .route(
            "/panel/api/server/updateGeofile",
            post(compat::server_update_geofile_all),
        )
        .route(
            "/panel/api/server/updateGeofile/{file}",
            post(compat::server_update_geofile_one),
        )
        .route("/panel/api/server/logs/{rows}", post(compat::server_logs))
        .route(
            "/panel/api/server/restartXrayService",
            post(compat::server_restart_xray),
        )
        .route(
            "/panel/api/server/stopXrayService",
            post(compat::server_stop_xray),
        )
        .route(
            "/panel/api/server/panelUpdateCheck",
            get(compat::panel_update_check),
        )
        .route(
            "/panel/api/server/panelUpdate",
            post(compat::panel_update_run),
        )
        .route(
            "/panel/api/setting/updateUser",
            post(compat::setting_update_user),
        )
        .route(
            "/panel/api/setting/restartPanel",
            post(compat::setting_restart_panel),
        )
        .route("/panel/api/server/getNewUUID", get(compat::server_new_uuid))
        .route(
            "/panel/api/server/getNewX25519Cert",
            get(compat::server_new_x25519),
        )
        .route(
            "/panel/api/server/getNewmldsa65",
            get(compat::server_new_mldsa65),
        )
        .route(
            "/panel/api/server/getNewmlkem768",
            get(compat::server_new_mlkem768),
        )
        .route(
            "/panel/api/server/getNewVlessEnc",
            get(compat::server_new_vless_enc),
        )
        .route(
            "/panel/api/server/getNewEchCert",
            post(compat::server_new_ech_cert),
        )
        .route(
            "/panel/api/server/getCertHash",
            post(compat::server_cert_hash),
        )
        .route("/panel/api/inbounds/list", get(compat::inbounds_list))
        .route("/panel/api/inbounds/list/slim", get(compat::inbounds_slim))
        .route("/panel/api/inbounds/get/{id}", get(compat::inbound_get))
        .route("/panel/api/inbounds/options", get(compat::inbounds_options))
        .route("/panel/api/inbounds/add", post(compat::inbound_add))
        .route("/panel/api/inbounds/import", post(compat::inbounds_import))
        .route(
            "/panel/api/inbounds/update/{id}",
            post(compat::inbound_update),
        )
        .route("/panel/api/inbounds/del/{id}", post(compat::inbound_delete))
        .route(
            "/panel/api/inbounds/setEnable/{id}",
            post(compat::inbound_set_enable),
        )
        .route(
            "/panel/api/inbounds/allLinks",
            get(compat::inbounds_all_links),
        )
        .route(
            "/panel/api/inbounds/{id}/fallbacks",
            get(compat::inbound_fallbacks_get).post(compat::inbound_fallbacks_put),
        )
        .route(
            "/panel/api/inbounds/{id}/resetTraffic",
            post(compat::inbound_reset_traffic),
        )
        .route(
            "/panel/api/inbounds/{id}/delAllClients",
            post(compat::inbound_del_all_clients),
        )
        .route(
            "/panel/api/inbounds/bulkDel",
            post(compat::inbounds_bulk_del),
        )
        .route(
            "/panel/api/inbounds/resetAllTraffics",
            post(compat::inbounds_reset_all_traffic),
        )
        .route("/panel/api/clients/list", get(compat::clients_list))
        .route("/panel/api/clients/list/paged", get(compat::clients_paged))
        .route("/panel/api/clients/add", post(compat::client_add))
        .route(
            "/panel/api/clients/bulkCreate",
            post(compat::clients_bulk_create),
        )
        .route("/panel/api/clients/get/{email}", get(compat::client_get))
        .route(
            "/panel/api/clients/resetTraffic/{email}",
            post(compat::client_reset_traffic),
        )
        .route(
            "/panel/api/clients/resetAllTraffics",
            post(compat::clients_reset_all_traffic),
        )
        .route(
            "/panel/api/clients/bulkEnable",
            post(compat::clients_bulk_enable),
        )
        .route(
            "/panel/api/clients/bulkDisable",
            post(compat::clients_bulk_disable),
        )
        .route("/panel/api/clients/bulkDel", post(compat::clients_bulk_del))
        .route(
            "/panel/api/clients/bulkAdjust",
            post(compat::clients_bulk_adjust),
        )
        .route(
            "/panel/api/clients/bulkAttach",
            post(compat::clients_bulk_attach),
        )
        .route(
            "/panel/api/clients/bulkDetach",
            post(compat::clients_bulk_detach),
        )
        .route(
            "/panel/api/clients/groups/bulkAdd",
            post(compat::clients_groups_bulk_add),
        )
        .route(
            "/panel/api/clients/groups/bulkRemove",
            post(compat::clients_groups_bulk_remove),
        )
        .route(
            "/panel/api/clients/{email}/attach",
            post(compat::client_attach),
        )
        .route(
            "/panel/api/clients/{email}/detach",
            post(compat::client_detach),
        )
        .route(
            "/panel/api/clients/delDepleted",
            post(compat::clients_del_depleted),
        )
        .route(
            "/panel/api/clients/delOrphans",
            post(compat::clients_del_orphans),
        )
        .route("/panel/api/clients/export", get(compat::clients_export))
        .route("/panel/api/clients/import", post(compat::clients_import))
        .route(
            "/panel/api/clients/subLinks/{sub_id}",
            get(compat::client_sub_links),
        )
        .route(
            "/panel/api/clients/update/{email}",
            post(compat::client_update),
        )
        .route(
            "/panel/api/clients/del/{email}",
            post(compat::client_delete).delete(compat::client_delete),
        )
        .route("/panel/api/clients/onlines", post(compat::clients_onlines))
        .route(
            "/panel/api/clients/onlinesByGuid",
            post(compat::clients_onlines_by_guid),
        )
        .route(
            "/panel/api/clients/onlineInbounds",
            post(compat::clients_online_inbounds),
        )
        .route(
            "/panel/api/clients/activeInbounds",
            post(compat::clients_active_inbounds),
        )
        .route(
            "/panel/api/clients/lastOnline",
            post(compat::clients_last_online),
        )
        .route("/panel/api/clients/groups", get(compat::clients_groups))
        .route("/panel/api/hosts/list", get(compat::hosts_list))
        .route(
            "/panel/api/xray/",
            get(compat::xray_config).post(compat::xray_config),
        )
        .route("/panel/api/xray/update", post(compat::xray_update))
        .route(
            "/panel/api/xray/balancerStatus",
            post(compat::xray_balancer_status),
        )
        .route(
            "/panel/api/xray/balancerOverride",
            post(compat::xray_balancer_override),
        )
        .route(
            "/panel/api/xray/resetOutboundsTraffic",
            post(compat::xray_reset_outbound_traffic),
        )
        .route(
            "/panel/api/xray/testOutbounds",
            post(outbound_test::test_outbounds),
        )
        .route(
            "/panel/api/xray/outbound-subs",
            get(outbound_subs::list).post(outbound_subs::create),
        )
        .route(
            "/panel/api/xray/outbound-subs/parse",
            post(outbound_subs::parse_preview),
        )
        .route(
            "/panel/api/xray/outbound-subs/{id}",
            post(outbound_subs::update).delete(outbound_subs::delete),
        )
        .route(
            "/panel/api/xray/outbound-subs/{id}/refresh",
            post(outbound_subs::refresh),
        )
        .route(
            "/panel/api/xray/outbound-subs/{id}/move",
            post(outbound_subs::move_item),
        )
        .route(
            "/panel/api/xray/outbound-subs/{id}/del",
            post(outbound_subs::delete),
        )
        .route(
            "/panel/api/xray/getOutboundsTraffic",
            get(compat::xray_outbounds_traffic),
        )
        .route(
            "/panel/api/xray/getDefaultJsonConfig",
            get(compat::xray_default_config).post(compat::xray_default_config),
        )
        .route("/panel/api/{*rest}", any(compat::stub))
        .route("/panel/api", any(compat::stub))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let app = Router::new()
        .merge(public)
        .merge(protected)
        .fallback(web::static_handler)
        .with_state(state.clone());

    // The prefix must be stripped before the inner router matches paths: a
    // layer on the same router runs after route selection, so the rewritten
    // URI would reach only the 404 fallback. Wrapping the router as a fallback
    // service gives the guard a fresh routing pass on the stripped path.
    Router::new()
        .fallback_service(app)
        .layer(middleware::from_fn_with_state(state, web::base_path::guard))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
