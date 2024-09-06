use anyhow::Context;
use std::fmt::Display;
use zbus::Connection;
use zbus_polkit::policykit1::{self, CheckAuthorizationFlags};

#[derive(Debug, Clone)]
pub struct Package {
    pub path: String,
    pub name: String,
    pub is_installed: bool,
}

pub async fn grant_permissions(package: Package) -> Result<bool, zbus::fdo::Error> {
    let connection = Connection::system().await?;
    let polkit = policykit1::AuthorityProxy::new(&connection).await?;

    let pid = std::process::id();

    let permitted = if pid == 0 {
        true
    } else {
        let subject = zbus_polkit::policykit1::Subject::new_for_owner(pid, None, None)
            .context("could not create policykit1 subject")
            .map_err(zbus_error_from_display)?;

        polkit
            .check_authorization(
                &subject,
                "org.debian.apt.install-file",
                &std::collections::HashMap::new(),
                CheckAuthorizationFlags::AllowUserInteraction.into(),
                "",
            )
            .await
            .context("could not check policykit authorization")
            .map_err(zbus_error_from_display)?
            .is_authorized
    };

    if permitted {
        if let Ok(status) = install_file(&connection, package).await {
            Ok(status)
        } else {
            Err(zbus_error_from_display("Error during installation"))
        }
    } else {
        Err(zbus_error_from_display("Operation not permitted by Polkit"))
    }
}

fn zbus_error_from_display<E: Display>(why: E) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(format!("{}", why))
}

async fn install_file(connection: &Connection, package: Package) -> Result<bool, zbus::fdo::Error> {
    if let Ok(path) = connection
        .call_method(
            Some("org.debian.apt"),
            "/org/debian/apt",
            Some("org.debian.apt"),
            "InstallFile",
            &(package.path, false),
        )
        .await?
        .body()
        .deserialize::<&str>()
    {
        return match connection
            .call_method(
                Some("org.debian.apt"),
                path,
                Some("org.debian.apt.transaction"),
                "Run",
                &(),
            )
            .await
        {
            Ok(_) => return Ok(true),
            Err(_) => Err(zbus_error_from_display("Error running transaction")),
        };
    }

    Ok(false)
}
