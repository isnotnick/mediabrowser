use notify::{Watcher, RecursiveMode, EventKind};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use std::time::Duration;

pub enum WatcherEvent {
    Created(PathBuf),
    Removed(PathBuf),
    Modified(()),
}

pub fn start_watcher(dir: &Path) -> anyhow::Result<mpsc::Receiver<WatcherEvent>> {
    let (tx, rx) = mpsc::channel(100);
    
    let tx_clone = tx.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                let paths = event.paths.clone();
                for path in paths {
                    let ev = match event.kind {
                        EventKind::Create(_) => Some(WatcherEvent::Created(path)),
                        EventKind::Remove(_) => Some(WatcherEvent::Removed(path)),
                        EventKind::Modify(_) => Some(WatcherEvent::Modified(())),
                        _ => None,
                    };
                    
                    if let Some(e) = ev {
                        let _ = tx_clone.blocking_send(e);
                    }
                }
            },
            Err(e) => tracing::error!("Watch error: {:?}", e),
        }
    })?;

    watcher.watch(dir, RecursiveMode::Recursive)?;
    
    // We need to keep the watcher alive
    tokio::spawn(async move {
        // Keep the watcher in scope
        let _w = watcher;
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });

    Ok(rx)
}
