//! Desktop notifications for crashes and red health. Failures are logged and ignored.

pub fn show(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    let _ = std::thread::Builder::new()
        .name("orbitry-notify".into())
        .spawn(move || {
            let result = notify_rust::Notification::new()
                .appname("Orbitry")
                .summary(&title)
                .body(&body)
                .show();
            if let Err(err) = result {
                eprintln!("orbitry: notification: {err}");
            }
        });
}
