mod api;
#[cfg(feature = "server")]
mod auth;
mod clock;
#[cfg(feature = "server")]
mod config;
mod interpreter;
mod matching;
mod repository;
#[cfg(feature = "server")]
mod service;
mod types;
mod validation;

#[cfg(all(test, feature = "server"))]
mod http_e2e;
