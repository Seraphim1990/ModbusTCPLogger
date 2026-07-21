use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use axum::{middleware, Router};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use crate::api::router;
use crate::messages::main_msg::MainMsg;
use crate::api::web_sockets::{live_socket_unit::CoordUnitWebSocketCommand, live_socket::live_router,  web_sock_coord};
use tower_http::cors::CorsLayer;
use serde::{Deserialize, Serialize};
use crate::api::router::middlewares::{auth_middleware};




use crate::api::sockets::socket_addr;
use crate::logger;

#[derive(Clone)]
pub struct AppState {
    pub from_api: mpsc::Sender<MainMsg>,
    pub to_ws_coord: mpsc::Sender<CoordUnitWebSocketCommand>,
    pub ws_counter: Arc<AtomicUsize>
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub id: i32,
    pub role_id: i32,
    pub exp: i64,
    pub iat: i64,
}

pub const JWT_KEY: &[u8] = b"^dfqw)cxzc*F)NJKwerx^vxLeNlsd&";

pub async fn init_axum(from_api:  mpsc::Sender<MainMsg>, to_api: mpsc::Receiver<MainMsg>) {

        let addr = socket_addr::init_socket();
        let listener = TcpListener::bind(addr).await.expect("Помилка читання TcpListener");

        let (to_ws_coord, rx_ws_coord) = mpsc::channel(100);

        let app_state = AppState{
            from_api,
            to_ws_coord,
            ws_counter: Arc::new(AtomicUsize::new(0))
        };

        tokio::spawn(web_sock_coord::web_sock_coord(to_api, rx_ws_coord));

        let protected_router = Router::new()
            .merge(router::node::node_router())
            .merge(router::devices::devices_router())
            .merge(router::value::values_router())
            .merge(router::measures::measures_router())
            .merge(router::users::users_router())
            .merge(router::user_group::user_group_router())
            .merge(router::user_subgroups::user_subgroup_router())
            .merge(router::assign::assign_router())

            .layer(middleware::from_fn(auth_middleware));


        let app = Router::new()
            .merge(protected_router)
            .merge(router::auth::auth())
            .merge(router::root::root())
            .merge(live_router()) // "/live_data"
            .with_state(app_state)
            .layer(CorsLayer::permissive());

        logger::printers::event(format!("Сервер запущено: {}", addr));

        axum::serve(listener, app).await.unwrap();
}

