use std::collections::HashMap;

use anyhow::anyhow;
use packagekit_zbus::{
    zbus::{blocking::Connection, zvariant},
    PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};

#[derive(Debug)]
pub struct TransactionDetails {
    pub package_id: String,
    pub summary: String,
    pub description: String,
    pub url: String,
    pub license: String,
    pub size: String,
}

// https://github.com/PackageKit/PackageKit/blob/209aa62950e503494716fd046f8f5cb546bf57d4/lib/packagekit-glib2/pk-enum.h#L776-L798
#[allow(dead_code)]
#[repr(u64)]
enum TransactionFlag {
    None = 1 << 0,
    OnlyTrusted = 1 << 1,
    Simulate = 1 << 2,
    OnlyDownload = 1 << 3,
    AllowReinstall = 1 << 4,
    JustReinstall = 1 << 5,
    AllowDowngrade = 1 << 6,
    Last = 1 << 7,
}

#[derive(Debug)]
pub struct PackageKit {
    connection: Connection,
}

impl PackageKit {
    pub fn new() -> anyhow::Result<Self> {
        let conn = Connection::system()?;

        Ok(Self { connection: conn })
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

    pub fn install_packages_files(
        &self,
        files: &[&str],
        mut f: Box<dyn FnMut(u32) + 'static>,
    ) -> anyhow::Result<()> {
        let tx = self.transaction()?;
        tx.set_hints(&["interactive=true"])?;
        tx.set_hints(&["supports-plural-signals=true"])?;
        println!("installing packages {:?}", files);
        tx.install_files(TransactionFlag::None as u64, &files)?;
        let _tx_packages = transaction_handle(tx, |total_percentage| {
            f(total_percentage);
        })?;
        Ok(())
    }
}

pub fn transaction_handle(
    tx: TransactionProxyBlocking,
    mut on_progress: impl FnMut(u32),
) -> anyhow::Result<Vec<TransactionDetails>> {
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
                    return Err(anyhow!("{details} (error code {code})"));
                }
                "ItemProgress" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::ItemProgress
                    let (package_id, status, percentage) = signal.body::<(String, u32, u32)>()?;
                    println!("Status {status} {} {percentage}", package_id);
                    let total_percentage = tx.percentage().unwrap_or(percentage);
                    on_progress(total_percentage)
                }
                "Package" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::Package
                    let (info, package_id, _summary) = signal.body::<(u32, String, String)>()?;

                    println!("Info {info} {}", package_id);
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
