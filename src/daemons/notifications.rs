use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::broadcast;
use zbus::zvariant;

pub(super) async fn start() -> anyhow::Result<()> {
    let conn = super::session_connection().inner();

    let (sender, receiver) = broadcast::channel(128);
    let interface = NotificationServer {
        sender,
        receiver,
        id_counter: 0,
    };

    let inserted = conn
        .object_server()
        .at("/org/freedesktop/Notifications", interface)
        .await?;
    assert!(inserted, "Another notification daemon is running");

    Ok(())
}

pub fn get() -> zbus::blocking::object_server::InterfaceRef<NotificationServer> {
    let conn = super::session_connection();
    let obj_server = conn.object_server();
    let iface = obj_server
        .interface("/org/freedesktop/Notifications")
        .unwrap();
    iface
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: u32,
    pub app_name: Option<String>,
    pub app_icon: Option<String>,
    pub summary: String,
    pub body: Option<String>,
    // FIXME: HINTS
    pub timeout: Option<Duration>,
}

#[derive(Clone, Debug)]
pub enum Request {
    NewNotification {
        notification: Notification,
        replace: Option<u32>,
    },
    CloseNotification(u32),
}

struct NotificationServer {
    // We keep both the sender and receiver for the rest of the codebase
    // to get as many receivers as needed.
    sender: broadcast::Sender<Request>,
    receiver: broadcast::Receiver<Request>,
    // The ID counter.
    id_counter: u32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("fht-shell", "FHT", "0.0.0", "1.2")
    }

    async fn get_capabilities(&self) -> Vec<&str> {
        vec![
            // "actions",
            "body",
            "body-hyperlinks",
            "body-images",
            "body-markup",
            "icon-static",
        ]
    }

    async fn notify(
        &mut self,
        app_name: String,
        replace: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: HashMap<&str, zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let id = {
            self.id_counter += 1;
            self.id_counter
        };
        let notification = Notification {
            id,
            app_name: (!app_name.is_empty()).then_some(app_name),
            app_icon: (!app_icon.is_empty()).then_some(app_icon),
            summary,
            body: (!body.is_empty()).then_some(body),
            timeout: match expire_timeout {
                -1 => Some(Duration::from_secs(10)),
                0 => None,
                x => Some(Duration::from_secs(x as u64)),
            },
        };

        let replace = (replace != 0).then_some(replace);
        let _ = self
            .sender
            .send(Request::NewNotification {
                replace,
                notification,
            })
            .unwrap();

        id
    }

    async fn close_notification(&self, id: u32) {
        let _ = self.sender.send(Request::CloseNotification(id)).unwrap();
    }
}
