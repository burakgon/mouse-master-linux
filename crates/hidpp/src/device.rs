//! A HID++ 2.0 device reachable through a [`Channel`].

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Serialize;
use tracing::debug;

use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::features::id;
use crate::report::Params;

/// One entry of a device's feature table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FeatureEntry {
    pub id: u16,
    pub index: u8,
    pub version: u8,
    /// Feature type flags: 0x80 obsolete, 0x40 hidden (SW), 0x20 internal (engineering).
    pub flags: u8,
}

impl FeatureEntry {
    pub fn is_hidden(&self) -> bool {
        self.flags & 0x60 != 0
    }
}

/// The feature table of a device, indexed both ways.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FeatureTable {
    by_id: BTreeMap<u16, FeatureEntry>,
}

impl FeatureTable {
    pub fn get(&self, feature: u16) -> Option<&FeatureEntry> {
        self.by_id.get(&feature)
    }

    pub fn contains(&self, feature: u16) -> bool {
        self.by_id.contains_key(&feature)
    }

    pub fn by_index(&self, index: u8) -> Option<&FeatureEntry> {
        self.by_id.values().find(|e| e.index == index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &FeatureEntry> {
        self.by_id.values()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    fn insert(&mut self, entry: FeatureEntry) {
        self.by_id.insert(entry.id, entry);
    }
}

/// A HID++ 2.0 device: a channel, a device index and the feature table.
#[derive(Debug, Clone)]
pub struct Device {
    channel: Channel,
    index: u8,
    protocol: (u8, u8),
    features: FeatureTable,
}

impl Device {
    /// Pings the device and reads its feature table.
    pub async fn open(channel: Channel, index: u8) -> Result<Device> {
        let protocol = channel.ping(index).await?;
        if protocol.0 < 2 {
            return Err(Error::Protocol(format!("HID++ {}.{} devices are not supported", protocol.0, protocol.1)));
        }
        let mut device = Device { channel, index, protocol, features: FeatureTable::default() };
        device.features = device.read_feature_table().await?;
        debug!(index, ?protocol, features = device.features.len(), "opened HID++ device");
        Ok(device)
    }

    async fn read_feature_table(&self) -> Result<FeatureTable> {
        let mut table = FeatureTable::default();
        table.insert(FeatureEntry { id: id::ROOT, index: 0, version: 0, flags: 0 });
        let Some((fs_index, _, fs_version)) = self.root_get_feature(id::FEATURE_SET).await? else {
            return Err(Error::Protocol("device has no FeatureSet feature".into()));
        };
        table.insert(FeatureEntry { id: id::FEATURE_SET, index: fs_index, version: fs_version, flags: 0 });
        let count = self.channel.feature_call(self.index, fs_index, 0, &[]).await?.u8(0);
        for i in 1..=count {
            let reply = self.channel.feature_call(self.index, fs_index, 1, &[i]).await?;
            let entry = FeatureEntry { id: reply.u16(0), index: i, flags: reply.u8(2), version: reply.u8(3) };
            table.insert(entry);
        }
        Ok(table)
    }

    /// IRoot.getFeature: returns `(index, flags, version)` or `None` if unsupported.
    pub async fn root_get_feature(&self, feature: u16) -> Result<Option<(u8, u8, u8)>> {
        let reply = self.channel.feature_call(self.index, 0, 0, &feature.to_be_bytes()).await?;
        let index = reply.u8(0);
        if index == 0 && feature != id::ROOT { Ok(None) } else { Ok(Some((index, reply.u8(1), reply.u8(2)))) }
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn index(&self) -> u8 {
        self.index
    }

    pub fn protocol(&self) -> (u8, u8) {
        self.protocol
    }

    pub fn features(&self) -> &FeatureTable {
        &self.features
    }

    pub fn has(&self, feature: u16) -> bool {
        self.features.contains(feature)
    }

    pub fn feature_version(&self, feature: u16) -> Option<u8> {
        self.features.get(feature).map(|e| e.version)
    }

    /// Feature id for a feature index, used to dispatch events.
    pub fn feature_at(&self, index: u8) -> Option<u16> {
        self.features.by_index(index).map(|e| e.id)
    }

    fn index_of(&self, feature: u16) -> Result<u8> {
        self.features.get(feature).map(|e| e.index).ok_or(Error::FeatureNotSupported(feature))
    }

    /// Calls a feature function, retrying a few times while the device reports busy.
    pub async fn call(&self, feature: u16, function: u8, params: &[u8]) -> Result<Params> {
        let index = self.index_of(feature)?;
        let mut attempt = 0;
        loop {
            match self.channel.feature_call(self.index, index, function, params).await {
                Err(e) if e.is_busy() && attempt < 4 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(40 * attempt)).await;
                }
                other => return other,
            }
        }
    }

    /// Calls a feature function without waiting for a reply.
    pub async fn send(&self, feature: u16, function: u8, params: &[u8]) -> Result<()> {
        let index = self.index_of(feature)?;
        self.channel.feature_send(self.index, index, function, params).await
    }
}
