use zbus::blocking::Connection;

use crate::packagekit::{PackageKitModifyProxyBlocking, TransactionDetails};

#[derive(Debug, Clone)]
pub struct Package {
    pub path: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub summary: String,
    pub description: String,
    pub url: String,
    pub license: String,
    pub size: String,
}

impl Package {
    pub fn new(path: String, tx: TransactionDetails) -> Self {
        let mut parts = tx.package_id.split(';');
        let package_name = parts.next().unwrap_or("");
        let version = parts.next().unwrap_or("");
        let architecture = parts.next().unwrap_or("");

        Self {
            path,
            id: tx.package_id.clone(),
            name: package_name.to_string(),
            version: version.to_string(),
            architecture: architecture.to_string(),
            summary: tx.summary,
            description: tx.description,
            url: tx.url,
            license: tx.license,
            size: tx.size,
        }
    }
}

pub fn install_packages_local(package: Package) -> anyhow::Result<()> {
    let conn = Connection::session()?;

    if let Ok(proxy) = PackageKitModifyProxyBlocking::new(&conn) {
        proxy.install_package_files(0, &[&package.path], "show-confirm-search,hide-finished")?;
    }

    Ok(())
}
