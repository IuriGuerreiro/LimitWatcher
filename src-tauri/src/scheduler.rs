use tauri::{AppHandle, Emitter};
use tokio::time::{interval, Duration};

const REFRESH_INTERVAL_SECS: u64 = 300; // 5 minutes

pub async fn start(app: AppHandle) {
    let mut interval = interval(Duration::from_secs(REFRESH_INTERVAL_SECS));
    
    loop {
        interval.tick().await;
        
        // TODO: Refresh all enabled providers
        println!("Scheduler: Refreshing providers...");
        
        // Emit event to frontend about refresh
        let _ = app.emit("providers-refreshed", ());
    }
}
