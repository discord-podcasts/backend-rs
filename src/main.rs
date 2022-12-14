use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

use actix::{Actor, Context};
use actix_web::web::{self, Data};
use actix_web::{middleware::Logger, App, HttpServer};
use rand::Rng;
use tracing::info;
use tracing_subscriber::EnvFilter;

use podcast::PodcastData;

use crate::podcast::Podcast;

mod audio_server;
mod authentication;
mod podcast;
mod requests;
mod websocket;

pub struct Application {
    authentication: HashMap<u32, String>,
    sessions: Mutex<HashMap<u32, Podcast>>,
}

impl Application {
    fn new() -> Self {
        let mut auth = HashMap::new();
        auth.insert(123, "123".to_owned());
        auth.insert(345, "345".to_owned());
        Self {
            authentication: auth,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn generate_id(&self) -> u32 {
        let id: u32 = rand::thread_rng().gen();
        if self.sessions.lock().unwrap().contains_key(&id) {
            return self.generate_id();
        }
        id
    }

    fn sessions<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut HashMap<u32, Podcast>) -> R,
    {
        let mut sessions = self.sessions.lock().unwrap();
        f(&mut sessions)
    }

    fn with_session<F, R>(&self, id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut Podcast) -> R,
    {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(&id);
        match session {
            Some(session) => Some(f(session)),
            None => None,
        }
    }

    fn add_session(&self, podcast: Podcast) {
        self.sessions(|s| s.insert(podcast.data.id, podcast));
    }

    fn list_sessions(&self) -> Vec<PodcastData> {
        self.sessions(|sessions| {
            sessions
                .values()
                .map(|podcast| podcast.data.clone())
                .collect()
        })
    }

    fn remove_session(&self, id: u32) {
        self.sessions(|sessions| sessions.remove(&id));
    }
}

impl Actor for Application {
    type Context = Context<Application>;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let host = match env::var("HOST") {
        Ok(host) => host,
        Err(_) => "127.0.0.1".to_owned(),
    };
    let port = match env::var("PORT").map(|v| v.parse()) {
        Ok(Ok(port)) => port,
        _ => 5050,
    };
    info!("Launching application on {host}:{port}");

    let app = Data::new(Application::new());
    HttpServer::new(move || {
        let logger = Logger::default();
        App::new()
            .wrap(logger)
            .route("/podcast", web::get().to(podcast::get))
            .route("/podcast", web::post().to(podcast::create))
            .route("/list", web::get().to(podcast::list))
            .route("/ws", web::get().to(crate::websocket::ws::websocket))
            .app_data(Data::clone(&app))
    })
    .bind((host, port))?
    .run()
    .await
}
