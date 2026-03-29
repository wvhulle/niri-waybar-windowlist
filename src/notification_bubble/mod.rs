use std::ops::Deref;

use async_channel::Sender;
use futures::{Stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use serde::{Deserialize, Deserializer};
use thiserror::Error;
use waybar_cffi::gtk::glib;
use zbus::{
    fdo::{self, MonitoringProxy},
    names::{self as zbus_names, InterfaceName, MemberName},
    zvariant::{DeserializeDict, Optional, Type},
    Connection, MatchRule, Message, MessageStream,
};

use crate::niri::WindowInfo;

pub(crate) use style::NotificationUrgency;

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
    /// Full PID ancestor chain, resolved eagerly before the sender exits.
    pid_chain: Vec<i64>,
}

impl NotificationData {
    pub fn get_notification(&self) -> &NotificationContent {
        &self.notification
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
    let connection = Connection::session().await?;
    let monitor_proxy = MonitoringProxy::new(&connection).await?;
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
    let mut message_stream = MessageStream::from(connection);
    while let Some(msg) = message_stream.try_next().await? {
        if let Err(e) = handle_message(&tx, &msg).await {
            tracing::error!(%e, "notification processing failed");
        }
    }

    Ok(())
}

async fn handle_message(
    tx: &Sender<NotificationData>,
    msg: &Message,
) -> Result<(), NotificationError> {
    if msg.header().interface() == Some(&InterfaceName::from_static_str(NOTIFICATION_INTERFACE)?)
        && msg.header().member() == Some(&MemberName::from_static_str(NOTIFY_METHOD)?)
    {
        let notification: NotificationContent = msg.body().deserialize()?;

        // Use the sender-pid hint (available immediately from the message body)
        // to walk the process tree synchronously before the sender exits.
        let pid_chain = resolve_pid_chain(notification.hints.sender_pid);

        tracing::debug!(
            app_name = ?notification.app_name,
            summary = %notification.summary,
            ?pid_chain,
            "received notification"
        );

        tx.send(NotificationData {
            notification,
            pid_chain,
        })
        .await
        .map_err(|_| NotificationError::ChannelClosed)?;
    }

    Ok(())
}

/// Walk the process tree from `start_pid` upward synchronously, collecting all
/// ancestor PIDs. Uses synchronous `/proc` reads to avoid yielding while the
/// short-lived sender process is still alive.
fn resolve_pid_chain(start_pid: Option<i64>) -> Vec<i64> {
    let mut chain = Vec::new();
    let Some(mut pid) = start_pid else {
        return chain;
    };
    chain.push(pid);
    loop {
        match procfs::process::Process::new(i32::try_from(pid).unwrap_or(0))
            .and_then(|p| p.stat())
        {
            Ok(stat) if stat.ppid > 1 => {
                let parent = i64::from(stat.ppid);
                chain.push(parent);
                pid = parent;
            }
            _ => break,
        }
    }
    chain
}


// ── Notification → window matching ──

/// Returns the IDs of unfocused windows that match the notification by PID,
/// paired with the notification's urgency level.
pub(crate) fn match_notification(
    notification: &NotificationData,
    windows: &[WindowInfo],
) -> Vec<(u64, NotificationUrgency)> {
    let urgency = NotificationUrgency::from_hint(notification.get_notification().hints.urgency);
    let pid_set: std::collections::HashSet<i64> =
        notification.pid_chain.iter().copied().collect();

    windows
        .iter()
        .filter(|w| !w.is_focused)
        .filter(|w| w.pid.is_some_and(|pid| pid_set.contains(&i64::from(pid))))
        .map(|w| {
            tracing::debug!(window_id = w.id, "notification matched window via PID chain");
            (w.id, urgency)
        })
        .collect()
}
