pub mod calendar;
pub mod company_factory;
pub mod engine;
pub mod lifecycle;
pub mod pricing;
pub mod seed;

pub fn start_sim_loop(state: crate::state::AppState) {
    tokio::spawn(async move {
        engine::run_loop(state).await;
    });
}