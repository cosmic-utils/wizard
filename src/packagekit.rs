use std::collections::HashMap;

use packagekit_zbus::{
    zbus::{blocking::Connection, zvariant},
    PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};
use zbus::proxy;

#[derive(Debug)]
pub struct TransactionDetails {
    pub package_id: String,
    pub summary: String,
    pub description: String,
    pub url: String,
    pub license: String,
    pub size: String,
}

pub struct PackageKit {
    connection: Connection,
}

impl PackageKit {
    pub fn new() -> Self {
        let conn = Connection::system().unwrap();

        Self { connection: conn }
    }

    pub fn _proxy(&self) -> anyhow::Result<PackageKitProxyBlocking> {
        Ok(PackageKitProxyBlocking::new(&self.connection)?)
    }

    pub fn transaction(&self) -> anyhow::Result<TransactionProxyBlocking> {
        let pk = PackageKitProxyBlocking::new(&self.connection)?;
        let tx_path = pk.create_transaction()?;
        let tx = TransactionProxyBlocking::builder(&self.connection)
            .destination("org.freedesktop.PackageKit")?
            .path(tx_path)?
            .build()?;

        Ok(tx)
    }
}

pub fn transaction_handle(tx: TransactionProxyBlocking) -> anyhow::Result<Vec<TransactionDetails>> {
    let mut details = Vec::new();

    for signal in tx.receive_all_signals()? {
        if let Some(member) = signal.member() {
            match member.as_str() {
                "Details" => {
                    let map = signal.body::<HashMap<String, zvariant::Value>>()?;
                    let get_string = |key: &str| -> Option<String> {
                        match map.get(key) {
                            Some(zvariant::Value::Str(str)) => Some(str.to_string()),
                            unknown => {
                                println!(
                                        "failed to find string for key {:?} in packagekit Details: found {:?} instead",
                                        key,
                                        unknown
                                    );
                                None
                            }
                        }
                    };
                    let size = match map.get("size") {
                        Some(zvariant::Value::U64(number)) => {
                            let size_in_mb = number / 1_000_000;
                            format!("{} MB", size_in_mb)
                        }
                        _ => String::from("0 MB"),
                    };

                    let Some(package_id) = get_string("package-id") else {
                        continue;
                    };
                    let summary = get_string("summary").unwrap_or_default();
                    let description = get_string("description").unwrap_or_default();
                    let url = get_string("url").unwrap_or_default();
                    let license = get_string("license").unwrap_or_default();

                    details.push(TransactionDetails {
                        package_id,
                        summary,
                        description,
                        url,
                        license,
                        size,
                    });
                }
                "ErrorCode" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::ErrorCode
                    let (code, details) = signal.body::<(u32, String)>()?;
                    println!("{details} (code {code})");
                    break;
                }
                "Finished" => {
                    break;
                }
                _ => {
                    println!("unknown signal {}", member);
                }
            }
        }
    }
    Ok(details)
}

#[proxy(
    interface = "org.freedesktop.PackageKit.Modify",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKitModify {
    fn install_package_files(
        &self,
        xid: u32,
        files: &[&str],
        interaction: &str,
    ) -> zbus::Result<()>;
}
