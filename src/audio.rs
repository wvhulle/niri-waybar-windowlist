use std::collections::HashMap;

use async_channel::Sender;
use futures::{Stream, TryStreamExt};
use waybar_cffi::gtk::glib;
use zbus::{
    names::{BusName, InterfaceName, OwnedBusName},
    zvariant::{OwnedValue, Value},
    Connection, MatchRule, MessageStream,
};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// Bus suffixes that are proxy daemons, not real players.
const IGNORED_SUFFIXES: &[&str] = &["playerctld"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl PlaybackStatus {
    fn parse(s: &str) -> Self {
        match s {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }

    /// Higher priority status wins when merging multiple instances.
    fn priority(self) -> u8 {
        match self {
            Self::Playing => 2,
            Self::Paused => 1,
            Self::Stopped => 0,
        }
    }

    fn most_active(a: Self, b: Self) -> Self {
        if a.priority() >= b.priority() {
            a
        } else {
            b
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct AudioState {
    pub by_desktop_entry: HashMap<String, PlaybackStatus>,
    pub by_bus_suffix: HashMap<String, PlaybackStatus>,
}

/// Handle keeping the MPRIS monitor alive. Dropping stops monitoring.
pub struct AudioMonitor {
    _task: glib::JoinHandle<()>,
}

pub fn start() -> (AudioMonitor, impl Stream<Item = AudioState>) {
    let (tx, rx) = async_channel::unbounded::<AudioState>();

    let task = glib::spawn_future_local(run_monitor_with_reconnect(tx));
    let monitor = AudioMonitor { _task: task };

    let stream = async_stream::stream! {
        while let Ok(state) = rx.recv().await {
            yield state;
        }
    };

    (monitor, stream)
}

async fn run_monitor_with_reconnect(tx: Sender<AudioState>) {
    const MAX_BACKOFF_SECS: u64 = 30;
    let mut backoff_secs = 1u64;

    loop {
        match run_monitor(tx.clone()).await {
            Ok(()) => {
                tracing::info!("MPRIS monitor ended normally");
                return;
            }
            Err(e) => {
                tracing::warn!(%e, backoff_secs, "MPRIS monitor error, reconnecting");
                glib::timeout_future_seconds(u32::try_from(backoff_secs).unwrap_or(u32::MAX)).await;
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
            }
        }
    }
}

#[derive(Debug)]
struct TrackedPlayer {
    status: PlaybackStatus,
    desktop_entry: Option<String>,
    bus_suffix: String,
}

fn extract_bus_suffix(bus_name: &str) -> Option<String> {
    let after_prefix = bus_name.strip_prefix(MPRIS_PREFIX)?;
    // "spotify" → "spotify", "firefox.instance_1234" → "firefox"
    let suffix = after_prefix
        .split('.')
        .next()
        .unwrap_or(after_prefix)
        .to_lowercase();

    if IGNORED_SUFFIXES.contains(&suffix.as_str()) {
        return None;
    }

    Some(suffix)
}

async fn query_player_properties(
    connection: &Connection,
    bus_name: &BusName<'_>,
) -> Result<(PlaybackStatus, Option<String>), zbus::Error> {
    let proxy = zbus::fdo::PropertiesProxy::builder(connection)
        .destination(bus_name.clone())?
        .path("/org/mpris/MediaPlayer2")?
        .build()
        .await?;

    let player_iface = InterfaceName::from_static_str_unchecked(PLAYER_INTERFACE);
    let root_iface = InterfaceName::from_static_str_unchecked(MPRIS_ROOT_INTERFACE);

    let status = proxy
        .get(player_iface, "PlaybackStatus")
        .await
        .ok()
        .and_then(|v| String::try_from(v).ok())
        .map_or(PlaybackStatus::Stopped, |s| PlaybackStatus::parse(&s));

    let desktop_entry = proxy
        .get(root_iface, "DesktopEntry")
        .await
        .ok()
        .and_then(|v| String::try_from(v).ok())
        .map(|s| s.to_lowercase());

    Ok((status, desktop_entry))
}

fn build_audio_state(players: &HashMap<OwnedBusName, TrackedPlayer>) -> AudioState {
    let mut state = AudioState::default();

    for player in players.values() {
        // Merge by bus suffix
        state
            .by_bus_suffix
            .entry(player.bus_suffix.clone())
            .and_modify(|existing| {
                *existing = PlaybackStatus::most_active(*existing, player.status)
            })
            .or_insert(player.status);

        // Merge by desktop entry
        if let Some(ref entry) = player.desktop_entry {
            state
                .by_desktop_entry
                .entry(entry.clone())
                .and_modify(|existing| {
                    *existing = PlaybackStatus::most_active(*existing, player.status);
                })
                .or_insert(player.status);
        }
    }

    state
}

fn send_state(players: &HashMap<OwnedBusName, TrackedPlayer>, tx: &Sender<AudioState>) {
    let _ = tx.try_send(build_audio_state(players));
}

async fn run_monitor(tx: Sender<AudioState>) -> Result<(), zbus::Error> {
    tracing::info!("starting MPRIS audio monitor");

    let connection = Connection::session().await?;
    let mut players: HashMap<OwnedBusName, TrackedPlayer> = HashMap::new();

    // List existing MPRIS players
    let dbus_proxy = zbus::fdo::DBusProxy::new(&connection).await?;
    let names = dbus_proxy.list_names().await?;

    for name in &names {
        let name_str = name.as_str();
        if let Some(bus_suffix) = extract_bus_suffix(name_str) {
            let bus_name = BusName::from(name.clone());
            match query_player_properties(&connection, &bus_name).await {
                Ok((status, desktop_entry)) => {
                    tracing::info!(%name, ?status, ?desktop_entry, "found existing MPRIS player");
                    let owned: OwnedBusName = BusName::from(name.clone()).into();
                    players.insert(
                        owned,
                        TrackedPlayer {
                            status,
                            desktop_entry,
                            bus_suffix,
                        },
                    );
                }
                Err(e) => {
                    tracing::debug!(%name, %e, "failed to query MPRIS player");
                }
            }
        }
    }

    send_state(&players, &tx);

    // Subscribe to NameOwnerChanged for MPRIS player appear/disappear
    let name_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .build();
    dbus_proxy.add_match_rule(name_rule).await?;

    // Subscribe to PropertiesChanged on MPRIS Player interface
    let props_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(PROPERTIES_INTERFACE)?
        .member("PropertiesChanged")?
        .build();
    dbus_proxy.add_match_rule(props_rule).await?;

    let mut stream = MessageStream::from(connection.clone());

    tracing::info!("MPRIS monitor connected, listening for changes");

    while let Some(msg) = stream.try_next().await? {
        let header = msg.header();
        let Some(member) = header.member() else {
            continue;
        };
        let Some(interface) = header.interface() else {
            continue;
        };

        if interface.as_str() == "org.freedesktop.DBus" && member.as_str() == "NameOwnerChanged" {
            handle_name_changed(&msg, &connection, &mut players, &tx).await;
        } else if interface.as_str() == PROPERTIES_INTERFACE
            && member.as_str() == "PropertiesChanged"
        {
            handle_properties_changed(&msg, &dbus_proxy, &mut players, &tx).await;
        }
    }

    Ok(())
}

async fn handle_name_changed(
    msg: &zbus::Message,
    connection: &Connection,
    players: &mut HashMap<OwnedBusName, TrackedPlayer>,
    tx: &Sender<AudioState>,
) {
    let Ok(body): Result<(String, String, String), _> = msg.body().deserialize() else {
        return;
    };
    let (name, old_owner, new_owner) = body;

    if !name.starts_with(MPRIS_PREFIX) {
        return;
    }

    let Some(bus_suffix) = extract_bus_suffix(&name) else {
        return;
    };

    if new_owner.is_empty() {
        let Ok(bus_name): Result<OwnedBusName, _> = name.clone().try_into() else {
            return;
        };
        if players.remove(&bus_name).is_some() {
            tracing::info!(%name, "MPRIS player disappeared");
            send_state(players, tx);
        }
    } else if old_owner.is_empty() {
        let Ok(bus_name_ref) = BusName::try_from(name.as_str()) else {
            return;
        };
        match query_player_properties(connection, &bus_name_ref).await {
            Ok((status, desktop_entry)) => {
                tracing::info!(%name, ?status, ?desktop_entry, "MPRIS player appeared");
                let Ok(bus_name): Result<OwnedBusName, _> = name.try_into() else {
                    return;
                };
                players.insert(
                    bus_name,
                    TrackedPlayer {
                        status,
                        desktop_entry,
                        bus_suffix,
                    },
                );
                send_state(players, tx);
            }
            Err(e) => {
                tracing::debug!(%name, %e, "failed to query new MPRIS player");
            }
        }
    }
}

async fn handle_properties_changed(
    msg: &zbus::Message,
    dbus_proxy: &zbus::fdo::DBusProxy<'_>,
    players: &mut HashMap<OwnedBusName, TrackedPlayer>,
    tx: &Sender<AudioState>,
) {
    let header = msg.header();
    let Some(sender) = header.sender() else {
        return;
    };
    let sender_str = sender.as_str().to_owned();

    let Ok(body): Result<(String, HashMap<String, OwnedValue>, Vec<String>), _> =
        msg.body().deserialize()
    else {
        return;
    };
    let (changed_interface, changed_props, _invalidated) = body;

    let is_player = changed_interface == PLAYER_INTERFACE;
    let is_root = changed_interface == MPRIS_ROOT_INTERFACE;
    if !is_player && !is_root {
        return;
    }

    let mut updated = false;
    let player_names: Vec<OwnedBusName> = players.keys().cloned().collect();

    for player_bus_name in player_names {
        let owner = dbus_proxy
            .get_name_owner(player_bus_name.as_ref())
            .await
            .ok();
        if owner.as_ref().map(|o| o.as_str()) != Some(sender_str.as_str()) {
            continue;
        }
        if let Some(player) = players.get_mut(&player_bus_name) {
            if is_player {
                if let Some(status_val) = changed_props.get("PlaybackStatus") {
                    if let Ok(status_str) = <&str>::try_from(&Value::from(status_val.clone())) {
                        let new_status = PlaybackStatus::parse(status_str);
                        if player.status != new_status {
                            tracing::debug!(%player_bus_name, ?new_status, "playback status changed");
                            player.status = new_status;
                            updated = true;
                        }
                    }
                }
            }
            if let Some(entry_val) = changed_props.get("DesktopEntry") {
                if let Ok(entry_str) = <&str>::try_from(&Value::from(entry_val.clone())) {
                    player.desktop_entry = Some(entry_str.to_lowercase());
                    updated = true;
                }
            }
        }
        break;
    }

    if updated {
        send_state(players, tx);
    }
}
