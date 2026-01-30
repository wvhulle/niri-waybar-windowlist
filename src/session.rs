use futures::Stream;
use zbus::Connection;

/// Listens for logind session Unlock signals via D-Bus.
/// Yields `()` each time the current session is unlocked.
pub fn unlock_stream() -> impl Stream<Item = ()> {
    async_stream::stream! {
        let Ok(connection) = Connection::system().await else {
            tracing::error!("failed to connect to system D-Bus");
            return;
        };

        let session_path = match get_session_path(&connection).await {
            Ok(path) => path,
            Err(e) => {
                tracing::error!(%e, "failed to get logind session path");
                return;
            }
        };

        tracing::info!(session_path, "listening for session unlock signals");

        let rule = format!(
            "type='signal',interface='org.freedesktop.login1.Session',member='Unlock',path='{}'",
            session_path,
        );

        if let Err(e) = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "AddMatch",
                &rule,
            )
            .await
        {
            tracing::error!(%e, "failed to add D-Bus match rule for Unlock signal");
            return;
        }

        loop {
            match futures::StreamExt::next(&mut zbus::MessageStream::from(&connection)).await {
                Some(Ok(msg)) => {
                    let header = msg.header();
                    if header.member().map(|m| m.as_str()) == Some("Unlock")
                        && header.interface().map(|i| i.as_str()) == Some("org.freedesktop.login1.Session")
                    {
                        tracing::info!("session unlock signal received");
                        yield ();
                    }
                }
                Some(Err(e)) => {
                    tracing::warn!(%e, "D-Bus message stream error");
                }
                None => {
                    tracing::warn!("D-Bus message stream ended");
                    return;
                }
            }
        }
    }
}

async fn get_session_path(connection: &Connection) -> Result<String, zbus::Error> {
    let reply = connection
        .call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            "GetSession",
            &"auto",
        )
        .await?;

    let path: zbus::zvariant::OwnedObjectPath = reply.body().deserialize()?;
    Ok(path.to_string())
}
