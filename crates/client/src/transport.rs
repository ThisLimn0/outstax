use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[async_trait]
pub trait Transport: Send {
    fn is_ble(&self) -> bool {
        false
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<()>;
    async fn receive(&mut self) -> Result<Vec<u8>>;
    async fn disconnect(&mut self) -> Result<()>;
}

/// A raw Link byte stream carried by a local diagnostic bridge/emulator.
/// This is not the legacy SHARE Wi-Fi protocol.
pub struct TcpTransport {
    stream: TcpStream,
}
impl TcpTransport {
    pub async fn connect(address: &str) -> Result<Self> {
        let stream = TcpStream::connect(address).await?;
        stream.set_nodelay(true)?;
        log::info!("connected to TCP bridge {address}");
        Ok(Self { stream })
    }
}
#[async_trait]
impl Transport for TcpTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.stream.write_all(bytes).await?;
        Ok(())
    }
    async fn receive(&mut self) -> Result<Vec<u8>> {
        let mut bytes = vec![0; 4096];
        let count = self.stream.read(&mut bytes).await?;
        if count == 0 {
            log::error!("TCP bridge closed the connection");
            bail!("transport closed");
        }
        bytes.truncate(count);
        Ok(bytes)
    }
    async fn disconnect(&mut self) -> Result<()> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

#[async_trait]
impl Transport for Box<dyn Transport> {
    fn is_ble(&self) -> bool {
        self.as_ref().is_ble()
    }
    async fn send(&mut self, b: &[u8]) -> Result<()> {
        self.as_mut().send(b).await
    }
    async fn receive(&mut self) -> Result<Vec<u8>> {
        self.as_mut().receive().await
    }
    async fn disconnect(&mut self) -> Result<()> {
        self.as_mut().disconnect().await
    }
}
