#[zbus::proxy(
    interface = "org.debian.apt",
    default_service = "org.debian.apt",
    default_path = "/org/debian/apt"
)]
trait AptDaemon {
    fn install_file(&self, path: &str, force: bool) -> zbus::Result<String>;
}

#[zbus::proxy(
    interface = "org.debian.apt.transaction",
    default_service = "org.debian.apt"
)]
trait AptTransaction {
    fn run(&self) -> zbus::Result<()>;
}
