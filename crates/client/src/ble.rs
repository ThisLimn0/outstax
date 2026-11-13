use crate::Transport;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use btleplug::{
    api::{
        Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        ValueNotification, WriteType,
    },
    platform::{Manager, Peripheral},
};
use futures::{Stream, StreamExt};
use outstax_protocol::{NOTIFY_UUID, SERVICE_UUID, WRITE_UUID};
use std::{pin::Pin, time::Duration};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub address: String,
    pub rssi: Option<i16>,
    pub has_print_service: bool,
}
pub struct FoundDevice {
    pub info: Device,
    peripheral: Peripheral,
}

pub async fn discover(duration: Duration) -> Result<Vec<FoundDevice>> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    if adapters.is_empty() {
        log::error!("no Bluetooth adapter found");
        bail!("no Bluetooth adapters available");
    }
    log::info!("scanning {} adapter(s) for {duration:?}", adapters.len());
    let service = Uuid::parse_str(SERVICE_UUID)?;
    let mut found = Vec::new();
    for adapter in adapters {
        if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
            log::error!("start_scan failed: {e:?}");
            // Windows HRESULT 0x800710DF (ERROR_DEVICE_NOT_READY): the radio is switched off.
            if format!("{e:?}").to_ascii_uppercase().contains("800710DF") {
                bail!("Bluetooth is turned off. Turn it on in Windows Settings > Bluetooth & devices, then scan again.");
            }
            return Err(e.into());
        }
        tokio::time::sleep(duration).await;
        let peripherals = adapter.peripherals().await?;
        adapter.stop_scan().await?;
        for peripheral in peripherals {
            let Some(p) = peripheral.properties().await? else {
                continue;
            };
            let name = p.local_name.unwrap_or_default();
            let has_print_service = p.services.contains(&service);
            if !has_print_service && !name.to_ascii_uppercase().contains("INSTAX") {
                continue;
            }
            let info = Device {
                id: peripheral.id().to_string(),
                name,
                address: p.address.to_string(),
                rssi: p.rssi,
                has_print_service,
            };
            if !found.iter().any(|d: &FoundDevice| d.info.id == info.id) {
                log::info!(
                    "found {:?} id {} rssi {:?} print service {}",
                    info.name,
                    info.id,
                    info.rssi,
                    info.has_print_service
                );
                found.push(FoundDevice { info, peripheral });
            }
        }
    }
    log::info!("scan found {} printer(s)", found.len());
    Ok(found)
}

const DISCONNECTED: &str = "Printer disconnected (switched off, out of range or link dropped). Switch it on and try again. An interrupted print is never retried automatically.";

/// Windows reports a dropped link as RO_E_CLOSED (0x80000013) once btleplug has closed the device.
fn link_error(e: btleplug::Error) -> anyhow::Error {
    log::error!("BLE write failed: {e:?}");
    let text = format!("{e:?}").to_ascii_uppercase();
    if matches!(e, btleplug::Error::NotConnected) || text.contains("80000013") {
        anyhow::anyhow!(DISCONNECTED)
    } else {
        e.into()
    }
}

/// ATT payload for a negotiated MTU, never below the 23-byte default or above 512.
pub fn fragment_size(mtu: u16) -> usize {
    (mtu as usize).saturating_sub(3).clamp(20, 512)
}

pub struct BleTransport {
    peripheral: Peripheral,
    write: Characteristic,
    write_type: WriteType,
    notifications: Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    notify_uuid: Uuid,
    write_size: usize,
    delay: Duration,
    /// Last fragment size used, to log when Windows changes the MTU.
    fragment: usize,
}
impl BleTransport {
    pub async fn connect(
        selector: &str,
        scan_duration: Duration,
        write_size: usize,
        delay: Duration,
    ) -> Result<Self> {
        if write_size > 512 {
            bail!("write size must be 0 (negotiated ATT MTU - 3) or 1..512");
        }
        let devices = discover(scan_duration).await?;
        let matches: Vec<_> = devices
            .into_iter()
            .filter(|d| {
                d.info.id.eq_ignore_ascii_case(selector)
                    || d.info.name.eq_ignore_ascii_case(selector)
                    || d.info.address.eq_ignore_ascii_case(selector)
            })
            .collect();
        match matches.len() {
            1 => {}
            0 => {
                log::warn!("printer {selector:?} not among the scanned devices");
                bail!("Printer not found. Make sure it is switched on and nearby, then scan again.")
            }
            n => bail!("{n} printers match '{selector}'; select the exact ID from a scan."),
        }
        let device = matches.into_iter().next().unwrap();
        let peripheral = device.peripheral;
        log::info!("connecting to {:?} ({})", device.info.name, device.info.id);
        peripheral.connect().await.context(
            "Bluetooth connect failed. Close other printer apps and check Bluetooth permissions",
        )?;
        let result = async {
            peripheral.discover_services().await?;
            let write_uuid = Uuid::parse_str(WRITE_UUID)?;
            let notify_uuid = Uuid::parse_str(NOTIFY_UUID)?;
            let characteristics = peripheral.characteristics();
            let service_uuid = Uuid::parse_str(SERVICE_UUID)?;
            let write = characteristics
                .iter()
                .find(|c| c.uuid == write_uuid && c.service_uuid == service_uuid)
                .cloned()
                .context("printer does not expose the known Link write characteristic")?;
            let notify = characteristics
                .iter()
                .find(|c| c.uuid == notify_uuid && c.service_uuid == service_uuid)
                .cloned()
                .context("printer does not expose the known Link notify characteristic")?;
            let write_type = if write
                .properties
                .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
            {
                WriteType::WithoutResponse
            } else if write.properties.contains(CharPropFlags::WRITE) {
                WriteType::WithResponse
            } else {
                bail!("characteristic is not writable");
            };
            let notifications = peripheral.notifications().await?;
            peripheral.subscribe(&notify).await?;
            Ok((write, write_type, notifications, notify_uuid))
        }
        .await;
        match result {
            Ok((write, write_type, notifications, notify_uuid)) => {
                let mtu = peripheral.mtu();
                log::info!(
                    "connected: MTU {mtu}, fragment {} bytes, {write_type:?}",
                    match write_size {
                        0 => fragment_size(mtu),
                        n => n,
                    }
                );
                Ok(Self {
                    peripheral,
                    write,
                    write_type,
                    notifications,
                    notify_uuid,
                    write_size,
                    delay,
                    fragment: 0,
                })
            }
            Err(e) => {
                log::error!("connected but setup failed: {e:#}");
                let _ = peripheral.disconnect().await;
                Err(e)
            }
        }
    }
}
#[async_trait]
impl Transport for BleTransport {
    fn is_ble(&self) -> bool {
        true
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        // 0 follows the apps (BleDataService.sendMessage): fragment at negotiated MTU - 3,
        // read per frame because Windows may raise MaxPduSize after connecting.
        let size = match self.write_size {
            0 => fragment_size(self.peripheral.mtu()),
            n => n,
        };
        if size != self.fragment {
            if self.fragment != 0 {
                log::info!("fragment size changed {} -> {size} bytes", self.fragment);
            }
            self.fragment = size;
        }
        for fragment in bytes.chunks(size) {
            self.peripheral
                .write(&self.write, fragment, self.write_type)
                .await
                .map_err(link_error)?;
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
        }
        Ok(())
    }
    async fn receive(&mut self) -> Result<Vec<u8>> {
        // The notification stream does not end when the link drops; check the
        // OS connection state each second so a switched-off printer is reported
        // promptly instead of as a 15 s response timeout.
        loop {
            match tokio::time::timeout(Duration::from_secs(1), self.notifications.next()).await {
                Ok(Some(n)) if n.uuid == self.notify_uuid => return Ok(n.value),
                Ok(Some(_)) => {}
                Ok(None) => {
                    log::error!("BLE notification stream ended: link dropped");
                    bail!(DISCONNECTED)
                }
                Err(_) => {
                    if !self.peripheral.is_connected().await.unwrap_or(false) {
                        log::error!("BLE link reported disconnected while waiting for a reply");
                        bail!(DISCONNECTED)
                    }
                }
            }
        }
    }
    async fn disconnect(&mut self) -> Result<()> {
        log::info!("disconnecting");
        self.peripheral.disconnect().await?;
        Ok(())
    }
}
