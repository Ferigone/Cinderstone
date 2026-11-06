//! Actual dedicated server service-key owner and configured chat registry holder.
use super::*;
use mc121_rust::chat_crypto::{ServiceKeyRefresh, ServiceKeySet};
use tokio::io::AsyncReadExt;

const MAX_LOOPBACK_KEY_BYTES: usize = 1024 * 1024;

/// The choice is fixed for the server lifetime. A failed file read never
/// changes this variant or reaches the official HTTP fetch branch.
pub(super) enum ServiceKeySource {
    Official,
    LoopbackFile { path: PathBuf, initial: Option<ServiceKeySet> },
}

pub(super) fn validate_source_options(options: &Options) -> Result<()> {
    if options.loopback_service_keys.is_some() {
        ensure!(options.allow_offline && !options.online_mode && options.bind.is_loopback(),
            "--loopback-service-keys requires --allow-offline and a loopback bind; online/public binds are unsupported");
    }
    Ok(())
}

async fn read_loopback_keys(path: &Path) -> Result<ServiceKeySet> {
    let metadata = fs::metadata(path).await.context("inspect local services public-key file")?;
    ensure!(metadata.is_file(), "local services public keys must be a regular file");
    ensure!(metadata.len() <= MAX_LOOPBACK_KEY_BYTES as u64,
        "local services public-key file exceeds 1MiB");
    let file = fs::File::open(path).await.context("open local services public-key file")?;
    let opened_metadata = file.metadata().await.context("inspect opened services public-key file")?;
    ensure!(opened_metadata.is_file(), "opened services public keys are not a regular file");
    ensure!(opened_metadata.len() <= MAX_LOOPBACK_KEY_BYTES as u64,
        "opened services public-key file exceeds 1MiB");
    // Bound the bytes actually read as well as metadata: the file may grow
    // between checks, and metadata alone cannot enforce the allocation bound.
    let mut bounded = file.take(MAX_LOOPBACK_KEY_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    bounded.read_to_end(&mut bytes).await.context("read local services public-key file")?;
    ensure!(bytes.len() <= MAX_LOOPBACK_KEY_BYTES,
        "local services public-key data exceeds 1MiB");
    ServiceKeySet::from_keyset_json(&bytes).context("parse local services public-key file")
}

pub(super) async fn prepare_key_source(options: &Options) -> Result<ServiceKeySource> {
    validate_source_options(options)?;
    match &options.loopback_service_keys {
        None => Ok(ServiceKeySource::Official),
        Some(path) => {
            // Propagate initial read/parse failure before listener startup.
            // An empty successful set can only come from the real JSON parser.
            let initial = read_loopback_keys(path).await?;
            Ok(ServiceKeySource::LoopbackFile { path: path.clone(), initial: Some(initial) })
        }
    }
}

impl ServiceKeySource {
    fn kind(&self) -> &'static str {
        match self { Self::Official => "official-https", Self::LoopbackFile { .. } => "loopback-file" }
    }
    pub(super) fn require_bound_address(&self, address: SocketAddr) -> Result<()> {
        if matches!(self, Self::LoopbackFile { .. }) {
            ensure!(address.ip().is_loopback(), "local services public keys require an actual loopback listener");
        }
        Ok(())
    }
    async fn read(&mut self) -> Result<ServiceKeySet> {
        match self {
            Self::Official => ServiceKeySet::fetch_official().await,
            Self::LoopbackFile { path, initial } => match initial.take() {
                Some(keys) => Ok(keys),
                None => read_loopback_keys(path).await,
            },
        }
    }
}

pub(super) fn start_key_refresh(server: Arc<Server>, mut source: ServiceKeySource) {
    let source_kind = source.kind();
    // Synchronous selection evidence precedes Main's listening event. It says
    // which real source will run, not that a certificate has validated.
    println!("{}", json!({"event":"chat_service_key_source","source":source_kind,
        "network_refresh":matches!(&source, ServiceKeySource::Official)}));
    tokio::spawn(async move {
        let mut refresh = ServiceKeyRefresh::new();
        loop {
            if server.shutting_down.load(Ordering::Acquire) { return; }
            let fetched = match source.read().await {
                Ok(keys) => Some(keys),
                Err(error) => {
                    eprintln!("{}", json!({"event":"chat_service_keys_unavailable",
                        "source":source_kind,"reason":error.to_string()}));
                    None
                }
            };
            let delay = refresh.record_attempt(fetched);
            if let Some(keys) = refresh.current() {
                server.chat_keys.send_replace(Arc::new(keys.clone()));
            }
            server.chat_keys_attempted.send_replace(true);
            println!("{}", json!({"event":"chat_service_keys_attempt_complete",
                "source":source_kind,
                "profile_keys":server.chat_keys.borrow().profile_certificate_key_count(),
                "next_refresh_seconds":delay.as_secs()}));
            time::sleep(delay).await;
        }
    });
}

impl Server {
    pub(super) fn current_secure_chat_enforcement(&self) -> bool {
        self.options.enforce_secure_profile && self.options.online_mode
            && self.chat_keys.borrow().can_validate_profile_keys()
    }
    pub(super) async fn wait_first_chat_key_attempt(&self) -> Result<()> {
        let mut ready = self.chat_keys_attempted.subscribe();
        loop {
            if *ready.borrow_and_update() { return Ok(()); }
            ready.changed().await.context("service-key owner closed before first attempt")?;
        }
    }
    pub(super) async fn wait_secure_chat_enforcement(&self) -> Result<()> {
        if self.options.enforce_secure_profile && self.options.online_mode {
            self.wait_first_chat_key_attempt().await?;
        }
        Ok(())
    }
}

/// Resolve the real minecraft:chat holder from the registry bytes sent to this
/// client. An absent/omitted registry is not replaced with a guessed ID.
pub(super) fn captured_chat_type(configuration: &[Packet]) -> Result<i32> {
    captured_chat_type_named(configuration, "minecraft:chat")
}

pub(super) fn captured_chat_type_named(configuration: &[Packet], key: &str) -> Result<i32> {
    for packet in configuration.iter().filter(|packet| packet.id == 0x07) {
        let mut reader = Reader::new(&packet.payload);
        if reader.string(32767)? != "minecraft:chat_type" { continue; }
        let count = reader.varint()?;
        ensure!((0..=65536).contains(&count), "invalid chat registry entry count");
        let mut chat = None;
        for index in 0..count {
            let name = reader.string(32767)?;
            let present = reader.bool()?;
            if present {
                let (_, consumed) = Tag::decode_anonymous_prefix(reader.remaining())?;
                reader.bytes(consumed)?;
            }
            if name == key {
                ensure!(present, "chat holder has no configured serialized data");
                ensure!(chat.replace(index).is_none(), "duplicate chat registry key");
            }
        }
        reader.finish()?;
        return chat.with_context(|| format!("configured {key} holder missing"));
    }
    bail!("configured chat-type registry missing")
}
