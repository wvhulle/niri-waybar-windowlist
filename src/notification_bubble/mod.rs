use std::ops::Deref;

use async_channel::Sender;
use futures::{Stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use serde::{Deserialize, Deserializer};
use thiserror::Error;
use waybar_cffi::gtk::glib;
use zbus::{
    fdo::{self, DBusProxy, MonitoringProxy},
    names::{self as zbus_names, InterfaceName, MemberName},
    zvariant::{DeserializeDict, Optional, Type},
    Connection, MatchRule, Message, MessageStream,
};

#[derive(Error, Debug)]
enum NotificationError {
    #[error(transparent)]
    Zbus(#[from] zbus::Error),

    #[error(transparent)]
    ZbusFdo(#[from] fdo::Error),

    #[error(transparent)]
    ZbusNames(#[from] zbus_names::Error),

    #[error("notification channel closed")]
    ChannelClosed,
}

pub(crate) mod settings;
pub(crate) mod style;

pub(crate) async fn forward_events(tx: Sender<crate::waybar_module::EventMessage>) {
    let mut stream = Box::pin(create_stream());
    while let Some(notification) = StreamExt::next(&mut stream).await {
        if let Err(e) = tx
            .send(crate::waybar_module::EventMessage::Notification(Box::new(
                notification,
            )))
            .await
        {
            tracing::error!(%e, "failed to forward notification");
        }
    }
}

pub fn create_stream() -> impl Stream<Item = NotificationData> {
    let (tx, rx) = async_channel::unbounded();
    glib::spawn_future_local(run_monitor_with_reconnect(tx));

    async_stream::stream! {
        while let Ok(notification) = rx.recv().await {
            yield notification;
        }
    }
}

async fn run_monitor_with_reconnect(tx: Sender<NotificationData>) {
    const MAX_BACKOFF_SECS: u64 = 30;
    let mut backoff_secs = 1u64;

    loop {
        match run_monitor(tx.clone()).await {
            Ok(()) => {
                tracing::info!("notification monitor ended");
                return;
            }
            Err(e) => {
                tracing::warn!(%e, backoff_secs, "notification monitor error, reconnecting");
                glib::timeout_future_seconds(u32::try_from(backoff_secs).unwrap_or(u32::MAX)).await;
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotificationData {
    notification: NotificationContent,
    process_id: Option<u32>,
}

impl NotificationData {
    pub fn get_notification(&self) -> &NotificationContent {
        &self.notification
    }

    pub fn get_process_id(&self) -> Option<i64> {
        match self.process_id {
            Some(pid) => Some(pid.into()),
            None => self.notification.hints.sender_pid,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Type)]
pub struct NotificationContent {
    pub app_name: Optional<String>,
    pub replaces_id: Optional<u32>,
    pub app_icon: Optional<String>,
    pub summary: String,
    pub body: Optional<String>,
    pub actions: ActionList,
    pub hints: HintData,
    pub expire_timeout: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Type)]
#[zvariant(signature = "as")]
pub struct ActionList(Vec<ActionItem>);

impl Deref for ActionList {
    type Target = Vec<ActionItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ActionItem {
    pub id: String,
    pub localised: String,
}

impl<'de> Deserialize<'de> for ActionList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(
            Vec::<String>::deserialize(deserializer)?
                .into_iter()
                .tuples::<(_, _)>()
                .map(|(id, localised)| ActionItem { id, localised })
                .collect(),
        ))
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, DeserializeDict, Type)]
#[zvariant(rename_all = "kebab-case", signature = "a{sv}")]
pub struct HintData {
    pub desktop_entry: Option<String>,
    pub sender_pid: Option<i64>,
    pub urgency: Option<u8>,
}

static NOTIFICATION_INTERFACE: &str = "org.freedesktop.Notifications";
static NOTIFY_METHOD: &str = "Notify";

#[tracing::instrument(level = "TRACE", skip_all, err)]
async fn run_monitor(tx: Sender<NotificationData>) -> Result<(), NotificationError> {
    // Separate connection for PID lookups (the monitor connection cannot make
    // method calls).
    let lookup_connection = Connection::session().await?;
    let dbus_proxy = DBusProxy::new(&lookup_connection).await?;

    let monitor_connection = Connection::session().await?;
    let monitor_proxy = MonitoringProxy::new(&monitor_connection).await?;
    monitor_proxy
        .become_monitor(
            &[MatchRule::builder()
                .interface(NOTIFICATION_INTERFACE)?
                .member(NOTIFY_METHOD)?
                .build()],
            0,
        )
        .await?;

    tracing::info!("notification monitor connected");
    let mut message_stream = MessageStream::from(monitor_connection);
    while let Some(msg) = message_stream.try_next().await? {
        if let Err(e) = handle_message(&tx, &dbus_proxy, &msg).await {
            tracing::error!(%e, "notification processing failed");
        }
    }

    Ok(())
}

async fn handle_message(
    tx: &Sender<NotificationData>,
    dbus_proxy: &DBusProxy<'_>,
    msg: &Message,
) -> Result<(), NotificationError> {
    if msg.header().interface() == Some(&InterfaceName::from_static_str(NOTIFICATION_INTERFACE)?)
        && msg.header().member() == Some(&MemberName::from_static_str(NOTIFY_METHOD)?)
    {
        let process_id = resolve_sender_pid(dbus_proxy, msg).await;
        let notification: NotificationContent = msg.body().deserialize()?;

        tracing::debug!(
            app_name = ?notification.app_name,
            summary = %notification.summary,
            desktop_entry = ?notification.hints.desktop_entry,
            hint_pid = ?notification.hints.sender_pid,
            ?process_id,
            "received notification"
        );

        tx.send(NotificationData {
            notification,
            process_id,
        })
        .await
        .map_err(|_| NotificationError::ChannelClosed)?;
    }

    Ok(())
}

/// Resolve the PID of the D-Bus sender immediately to minimise the race
/// window between receiving the monitored message and the sender disconnecting.
async fn resolve_sender_pid(dbus_proxy: &DBusProxy<'_>, msg: &Message) -> Option<u32> {
    let header = msg.header();
    let sender = header.sender()?;
    match dbus_proxy
        .get_connection_unix_process_id(sender.clone().into())
        .await
    {
        Ok(pid) => Some(pid),
        Err(e) => {
            tracing::trace!(%e, %sender, "sender PID lookup failed (sender may have disconnected)");
            None
        }
    }
}
